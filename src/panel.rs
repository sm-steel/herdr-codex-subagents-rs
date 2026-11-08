//! A read-only activity panel; row selection never sends keys to Codex.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
    MouseEventKind,
};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::agent::{AgentRow, Freshness, Status, ThreadId};
use crate::rollout::Monitor;

const SPINNER: [&str; 16] = [
    "⡀", "⡄", "⡆", "⡇", "⣇", "⣧", "⣷", "⣿", "⢿", "⢻", "⢹", "⢸", "⠸", "⠘", "⠈", "⠀",
];

/// Render and poll the observer until the user quits.
///
/// # Errors
/// Returns terminal setup, drawing, and input errors; terminal restoration is RAII.
pub fn run(mut monitor: Monitor) -> io::Result<()> {
    let stop = AtomicBool::new(false);
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::scope(|scope| {
        scope.spawn(|| observe_loop(&mut monitor, &stop, &sender));
        let _shutdown = StopOnDrop(&stop);
        ratatui::run(|terminal| {
            let _mouse = MouseCapture::new()?;
            ui_loop(terminal, &receiver)
        })
    })
}

struct StopOnDrop<'a>(&'a AtomicBool);
impl Drop for StopOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

struct Observation {
    rows: Vec<AgentRow>,
    warnings: usize,
    activity: BTreeMap<ThreadId, SystemTime>,
}

fn observe_loop(monitor: &mut Monitor, stop: &AtomicBool, sender: &mpsc::SyncSender<Observation>) {
    let mut discovery = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        if discovery.elapsed() >= Duration::from_secs(2) {
            let _ = monitor.discover();
            discovery = Instant::now();
        }
        let snapshot = Observation {
            rows: monitor.poll(),
            warnings: monitor.warnings(),
            activity: monitor.activity_times(),
        };
        if let Err(mpsc::TrySendError::Disconnected(_)) = sender.try_send(snapshot) {
            break;
        }
        for _ in 0..25 {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}

// Catppuccin Mocha: base, mantle, surface0, overlay0, subtext0, text, blue.
const BASE: Color = Color::Rgb(30, 30, 46);
const MANTLE: Color = Color::Rgb(24, 24, 37);
const SURFACE: Color = Color::Rgb(49, 50, 68);
const GRAY: Color = Color::Rgb(108, 112, 134);
const LEAF: Color = Color::Rgb(69, 71, 90);
const SUBTEXT: Color = Color::Rgb(166, 173, 200);
const TEXT: Color = Color::Rgb(205, 214, 244);
const WORKING: Color = Color::Rgb(137, 180, 250);

struct MouseCapture;
impl MouseCapture {
    #[expect(
        clippy::disallowed_methods,
        reason = "terminal adapter owns mouse capture"
    )]
    fn new() -> io::Result<Self> {
        crossterm::execute!(io::stdout(), EnableMouseCapture)?;
        Ok(Self)
    }
}
impl Drop for MouseCapture {
    #[expect(
        clippy::disallowed_methods,
        reason = "terminal adapter restores mouse capture"
    )]
    fn drop(&mut self) {
        let _ = crossterm::execute!(io::stdout(), DisableMouseCapture);
    }
}

struct TreeNode {
    source: usize,
    depth: usize,
    active: bool,
    children: bool,
    collapsed: bool,
    branch: String,
    continuation: String,
}

struct TreeView<'a> {
    rows: &'a [AgentRow],
    children: BTreeMap<usize, Vec<usize>>,
    active: BTreeSet<usize>,
    latest: BTreeMap<usize, SystemTime>,
    collapsed: &'a BTreeSet<ThreadId>,
}

impl<'a> TreeView<'a> {
    fn new(
        rows: &'a [AgentRow],
        collapsed: &'a BTreeSet<ThreadId>,
        activity: &BTreeMap<ThreadId, SystemTime>,
    ) -> Self {
        let mut children: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        let mut stack = Vec::new();
        for (index, row) in rows.iter().enumerate() {
            stack.truncate(row.depth());
            if let Some(parent) = stack.last() {
                children.entry(*parent).or_default().push(index);
            }
            stack.push(index);
        }
        let mut active = BTreeSet::new();
        for (index, row) in rows.iter().enumerate().rev() {
            if row.status() == Status::Working
                || children
                    .get(&index)
                    .is_some_and(|items| items.iter().any(|child| active.contains(child)))
            {
                active.insert(index);
            }
        }
        let mut latest: BTreeMap<usize, SystemTime> = BTreeMap::new();
        for (index, row) in rows.iter().enumerate().rev() {
            let time = activity
                .get(row.thread())
                .copied()
                .into_iter()
                .chain(
                    children
                        .get(&index)
                        .into_iter()
                        .flatten()
                        .filter_map(|child| latest.get(child).copied()),
                )
                .max();
            if let Some(time) = time {
                latest.insert(index, time);
            }
        }
        Self {
            rows,
            children,
            active,
            latest,
            collapsed,
        }
    }

    fn flatten(&self) -> Vec<TreeNode> {
        let mut result = Vec::new();
        if !self.rows.is_empty() {
            self.walk(0, &[], &mut result);
        }
        result
    }

    fn walk(&self, index: usize, stems: &[bool], result: &mut Vec<TreeNode>) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        let children = self.children.get(&index).cloned().unwrap_or_default();
        let collapsed = self.collapsed.contains(row.thread());
        let (branch, mut continuation) = connectors(stems);
        continuation.push_str(if !children.is_empty() && !collapsed {
            "│ "
        } else {
            "  "
        });
        result.push(TreeNode {
            source: index,
            depth: stems.len(),
            active: self.active.contains(&index),
            children: !children.is_empty(),
            collapsed,
            branch,
            continuation,
        });
        if collapsed {
            return;
        }
        let mut sorted = children;
        sorted.sort_by(|left, right| {
            self.active
                .contains(right)
                .cmp(&self.active.contains(left))
                .then_with(|| self.latest.get(right).cmp(&self.latest.get(left)))
        });
        for (position, child) in sorted.iter().enumerate() {
            let mut child_stems = stems.to_vec();
            child_stems.push(position + 1 < sorted.len());
            self.walk(*child, &child_stems, result);
        }
    }
}

fn connectors(stems: &[bool]) -> (String, String) {
    let mut branch = String::new();
    let mut continuation = String::new();
    for (index, more) in stems.iter().enumerate() {
        let stem = if *more { "│   " } else { "    " };
        branch.push_str(if index + 1 == stems.len() {
            if *more { "├── " } else { "└── " }
        } else {
            stem
        });
        continuation.push_str(stem);
    }
    (branch, continuation)
}

#[derive(Default)]
struct ViewState {
    selected: Option<ThreadId>,
    collapsed: BTreeSet<ThreadId>,
    seen: BTreeSet<ThreadId>,
    scroll: usize,
}

impl ViewState {
    fn remember(&mut self, rows: &[AgentRow]) {
        for row in rows {
            if self.seen.insert(row.thread().clone()) && row.depth() > 0 {
                self.collapsed.insert(row.thread().clone());
            }
        }
    }
    fn selected_index(&self, rows: &[AgentRow], nodes: &[TreeNode]) -> usize {
        nodes
            .iter()
            .position(|node| {
                rows.get(node.source)
                    .is_some_and(|row| Some(row.thread()) == self.selected.as_ref())
            })
            .unwrap_or(0)
    }
    fn select(&mut self, rows: &[AgentRow], nodes: &[TreeNode], index: usize) {
        self.selected = nodes
            .get(index)
            .and_then(|node| rows.get(node.source))
            .map(|row| row.thread().clone());
    }
    fn toggle(&mut self, rows: &[AgentRow], node: &TreeNode) {
        if !node.children {
            return;
        }
        if let Some(row) = rows.get(node.source)
            && !self.collapsed.remove(row.thread())
        {
            self.collapsed.insert(row.thread().clone());
        }
    }
    fn key(&mut self, code: KeyCode, rows: &[AgentRow], nodes: &[TreeNode]) {
        let index = self.selected_index(rows, nodes);
        let Some(node) = nodes.get(index) else {
            return;
        };
        match code {
            KeyCode::Down => {
                self.select(rows, nodes, (index + 1).min(nodes.len().saturating_sub(1)));
            }
            KeyCode::Up => self.select(rows, nodes, index.saturating_sub(1)),
            KeyCode::Home => self.select(rows, nodes, 0),
            KeyCode::End => self.select(rows, nodes, nodes.len().saturating_sub(1)),
            KeyCode::Enter | KeyCode::Char(' ') => self.toggle(rows, node),
            KeyCode::Left if node.children && !node.collapsed => self.toggle(rows, node),
            KeyCode::Left => {
                if let Some(parent) = nodes
                    .iter()
                    .take(index)
                    .rposition(|parent| parent.depth < node.depth)
                {
                    self.select(rows, nodes, parent);
                }
            }
            KeyCode::Right if node.collapsed => self.toggle(rows, node),
            KeyCode::Right if node.children => self.select(rows, nodes, index + 1),
            _ => {}
        }
    }
    fn keep_visible(&mut self, index: usize, capacity: usize) {
        if index == 0 {
            return;
        }
        let position = index - 1;
        if position < self.scroll {
            self.scroll = position;
        }
        if position >= self.scroll + capacity {
            self.scroll = (position + 1).saturating_sub(capacity);
        }
    }
}

fn ui_loop(
    terminal: &mut ratatui::DefaultTerminal,
    receiver: &mpsc::Receiver<Observation>,
) -> io::Result<()> {
    let mut rows = Vec::new();
    let mut activity = BTreeMap::new();
    let mut warnings = 0;
    let mut frame_index = 0;
    let mut state = ViewState::default();
    loop {
        if let Ok(snapshot) = receiver.try_recv() {
            rows = snapshot.rows;
            activity = snapshot.activity;
            state.remember(&rows);
            warnings = snapshot.warnings;
        }
        let nodes = TreeView::new(&rows, &state.collapsed, &activity).flatten();
        let selected = state.selected_index(&rows, &nodes);
        let mut body_height = 0;
        terminal.draw(|frame| {
            let [body, footer] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(frame.area());
            body_height = body.height;
            let capacity = usize::from(body.height / 4).saturating_sub(1).max(1);
            state.keep_visible(selected, capacity);
            frame.render_widget(
                Paragraph::new(render_rows(&rows, &nodes, &state, frame_index, body.width))
                    .style(Style::default().fg(TEXT).bg(BASE)),
                body,
            );
            frame.render_widget(
                Paragraph::new(format!(
                    "↑↓ select · ←→ fold · click toggle\nq quit · {warnings} diag · read only"
                ))
                .style(Style::default().fg(SUBTEXT).bg(MANTLE)),
                footer,
            );
        })?;
        frame_index = (frame_index + 1) % SPINNER.len();
        if !event::poll(Duration::from_millis(150))? {
            continue;
        }
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if key.code == KeyCode::Char('q')
                    || (key.code == KeyCode::Char('c')
                        && key.modifiers.contains(event::KeyModifiers::CONTROL))
                {
                    return Ok(());
                }
                state.key(key.code, &rows, &nodes);
            }
            Event::Mouse(mouse)
                if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                    && mouse.row < body_height =>
            {
                let slot = usize::from(mouse.row / 4);
                let index = if slot == 0 { 0 } else { state.scroll + slot };
                if let Some(node) = nodes.get(index) {
                    state.select(&rows, &nodes, index);
                    state.toggle(&rows, node);
                }
            }
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::ScrollDown => {
                state.key(KeyCode::Down, &rows, &nodes);
            }
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::ScrollUp => {
                state.key(KeyCode::Up, &rows, &nodes);
            }
            _ => {}
        }
    }
}

fn render_rows(
    rows: &[AgentRow],
    nodes: &[TreeNode],
    state: &ViewState,
    frame: usize,
    width: u16,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (index, node) in nodes
        .iter()
        .enumerate()
        .filter(|(index, _)| *index == 0 || *index > state.scroll)
    {
        let Some(row) = rows.get(node.source) else {
            continue;
        };
        let selected = index == state.selected_index(rows, nodes);
        let icon = if node.active {
            SPINNER.get(frame % SPINNER.len()).copied().unwrap_or("⡇")
        } else {
            "⣿"
        };
        let freshness_note = if row.freshness() == Freshness::Stale {
            " [stale]"
        } else {
            ""
        };
        let fold = if index == 0 {
            ""
        } else if !node.children {
            "─ "
        } else if node.collapsed {
            "▶ "
        } else {
            "▼ "
        };
        let branch = if node.branch.is_empty() {
            node.branch.clone()
        } else {
            format!("{}─", node.branch.trim_end())
        };
        let mut spans = vec![
            Span::styled(branch, Style::default().fg(GRAY)),
            Span::styled(
                format!("{icon} "),
                Style::default().fg(if node.active { WORKING } else { GRAY }),
            ),
            Span::styled(
                fold,
                Style::default().fg(if node.children { GRAY } else { LEAF }),
            ),
            Span::raw(format!("{}{freshness_note}", row.name())),
        ];
        if index == 0 {
            append_counts(&mut spans, rows, width);
        }
        let mut heading = Line::from(spans);
        if selected {
            heading = heading.style(Style::default().bg(SURFACE));
        }
        lines.push(heading);
        for (label, text) in [("Task", row.assignment()), ("Last", row.activity())] {
            lines.push(Line::from(vec![
                Span::styled(node.continuation.clone(), Style::default().fg(GRAY)),
                Span::styled(format!("  {label}: "), Style::default().fg(SUBTEXT)),
                Span::raw(text.unwrap_or("unavailable").to_owned()),
            ]));
        }
        lines.push(Line::from(Span::styled(
            node.continuation.clone(),
            Style::default().fg(GRAY),
        )));
    }
    lines
}

fn append_counts(spans: &mut Vec<Span<'static>>, rows: &[AgentRow], width: u16) {
    let working = rows
        .iter()
        .skip(1)
        .filter(|row| row.status() == Status::Working)
        .count();
    let count = working.to_string();
    let tail = format!(" of {}]", rows.len().saturating_sub(1));
    let used: usize = spans.iter().map(Span::width).sum();
    let padding = usize::from(width).saturating_sub(used + 1 + count.len() + tail.len());
    spans.push(Span::raw(" ".repeat(padding)));
    spans.push(Span::raw("["));
    spans.push(Span::styled(
        count,
        Style::default().fg(if working > 0 { WORKING } else { GRAY }),
    ));
    spans.push(Span::raw(tail));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{AgentTree, Event as AgentEvent};

    fn fixture() -> Vec<AgentRow> {
        let root = ThreadId::new("root".into());
        let mut tree = AgentTree::new(root);
        for (child, parent) in [("A", "root"), ("B", "root"), ("C", "B")] {
            tree.apply(AgentEvent::Discover {
                thread: ThreadId::new(child.into()),
                parent: ThreadId::new(parent.into()),
                name: Some(format!("Worker-{child}")),
                assignment: None,
            })
            .unwrap();
        }
        tree.apply(AgentEvent::Lifecycle {
            thread: ThreadId::new("C".into()),
            turn: "one".into(),
            status: Status::Working,
        })
        .unwrap();
        tree.rows()
    }
    #[test]
    fn working_descendants_sort_their_branch_first_and_animate_ancestors() {
        let rows = fixture();
        let collapsed = BTreeSet::new();
        let nodes = TreeView::new(&rows, &collapsed, &BTreeMap::new()).flatten();
        let names: Vec<_> = nodes.iter().map(|node| rows[node.source].name()).collect();
        assert_eq!(names, ["Main", "Worker-B", "Worker-C", "Worker-A"]);
        assert!(nodes[0].active && nodes[1].active && nodes[2].active);
        assert!(!nodes[3].active);
        assert_eq!(nodes[1].continuation, "│   │ ");
        assert_eq!(nodes[2].branch, "│   └── ");
    }
    #[test]
    fn folding_keeps_active_branch_visible_and_keyboard_restores_children() {
        let rows = fixture();
        let mut state = ViewState::default();
        let nodes = TreeView::new(&rows, &state.collapsed, &BTreeMap::new()).flatten();
        state.key(KeyCode::Down, &rows, &nodes);
        state.key(KeyCode::Left, &rows, &nodes);
        let nodes = TreeView::new(&rows, &state.collapsed, &BTreeMap::new()).flatten();
        assert_eq!(nodes.len(), 3);
        assert!(nodes[1].active && nodes[1].collapsed);
        state.key(KeyCode::Right, &rows, &nodes);
        assert_eq!(
            TreeView::new(&rows, &state.collapsed, &BTreeMap::new())
                .flatten()
                .len(),
            4
        );
    }
    #[test]
    fn scrolling_children_keeps_main_visible() {
        let rows = fixture();
        let state = ViewState {
            scroll: 2,
            ..ViewState::default()
        };
        let nodes = TreeView::new(&rows, &state.collapsed, &BTreeMap::new()).flatten();
        let text = render_rows(&rows, &nodes, &state, 0, 36)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Main"));
        assert!(text.contains("Worker-A"));
        assert!(!text.contains("Worker-B"));
        assert!(!text.contains("Worker-C"));
    }
    #[test]
    fn main_counter_is_right_aligned_and_only_active_count_uses_working_color() {
        let rows = fixture();
        let state = ViewState::default();
        let nodes = TreeView::new(&rows, &state.collapsed, &BTreeMap::new()).flatten();
        let lines = render_rows(&rows, &nodes, &state, 3, 36);
        assert_eq!(lines[0].width(), 36);
        assert!(lines[0].to_string().starts_with("⡇ Main"));
        assert!(lines[0].to_string().ends_with("[1 of 3]"));
        assert_eq!(
            lines[0]
                .spans
                .iter()
                .find(|span| span.content == "1")
                .unwrap()
                .style
                .fg,
            Some(WORKING)
        );
        assert!(
            lines
                .iter()
                .any(|line| line.to_string().contains("─⣿ ─ Worker-A"))
        );
    }
    #[test]
    fn main_starts_open_and_nested_branches_stay_folded_until_user_expands_them() {
        let rows = fixture();
        let mut state = ViewState::default();
        state.remember(&rows);
        let nodes = TreeView::new(&rows, &state.collapsed, &BTreeMap::new()).flatten();
        assert_eq!(nodes.len(), 3);
        assert!(!nodes[0].collapsed);
        assert!(nodes[1].collapsed && nodes[1].active);
        state.key(KeyCode::Down, &rows, &nodes);
        state.key(KeyCode::Right, &rows, &nodes);
        state.remember(&rows);
        assert_eq!(
            TreeView::new(&rows, &state.collapsed, &BTreeMap::new())
                .flatten()
                .len(),
            4
        );
    }
    #[test]
    fn inactive_main_has_no_left_padding() {
        let rows = AgentTree::new(ThreadId::new("root".into())).rows();
        let state = ViewState::default();
        let nodes = TreeView::new(&rows, &state.collapsed, &BTreeMap::new()).flatten();
        let lines = render_rows(&rows, &nodes, &state, 0, 36);
        assert!(lines[0].to_string().starts_with("⣿ Main"));
    }
    #[test]
    fn sorts_working_first_then_last_activity_within_each_group() {
        let mut tree = AgentTree::new(ThreadId::new("root".into()));
        let mut activity = BTreeMap::new();
        for (name, seconds, working) in [
            ("old", 1, false),
            ("recent", 4, false),
            ("working-old", 2, true),
            ("working-recent", 3, true),
        ] {
            let id = ThreadId::new(name.into());
            tree.apply(AgentEvent::Discover {
                thread: id.clone(),
                parent: ThreadId::new("root".into()),
                name: Some(name.into()),
                assignment: None,
            })
            .unwrap();
            if working {
                tree.apply(AgentEvent::Lifecycle {
                    thread: id.clone(),
                    turn: "one".into(),
                    status: Status::Working,
                })
                .unwrap();
            }
            activity.insert(id, SystemTime::UNIX_EPOCH + Duration::from_secs(seconds));
        }
        let rows = tree.rows();
        let collapsed = BTreeSet::new();
        let nodes = TreeView::new(&rows, &collapsed, &activity).flatten();
        let names: Vec<_> = nodes.iter().map(|node| rows[node.source].name()).collect();
        assert_eq!(
            names,
            ["Main", "working-recent", "working-old", "recent", "old"]
        );
    }
}
