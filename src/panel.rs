//! A read-only activity panel; row selection never sends keys to Codex.

use std::io;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::agent::{AgentRow, Freshness, Status};
use crate::rollout::Monitor;

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
        ratatui::run(|terminal| ui_loop(terminal, &receiver))
    })
}

struct StopOnDrop<'a>(&'a AtomicBool);
impl Drop for StopOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

fn observe_loop(
    monitor: &mut Monitor,
    stop: &AtomicBool,
    sender: &mpsc::SyncSender<(Vec<AgentRow>, usize)>,
) {
    let mut discovery = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        if discovery.elapsed() >= Duration::from_secs(2) {
            let _ = monitor.discover();
            discovery = Instant::now();
        }
        if let Err(mpsc::TrySendError::Disconnected(_)) =
            sender.try_send((monitor.poll(), monitor.warnings()))
        {
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

fn ui_loop(
    terminal: &mut ratatui::DefaultTerminal,
    receiver: &mpsc::Receiver<(Vec<AgentRow>, usize)>,
) -> io::Result<()> {
    let mut rows = Vec::new();
    let mut warnings = 0;
    let mut frame_index = 0;
    let mut scroll = 0_usize;
    loop {
        if let Ok((snapshot, diagnostics)) = receiver.try_recv() {
            rows = snapshot;
            warnings = diagnostics;
        }
        terminal.draw(|frame| {
            let [title,body,footer]=Layout::vertical([Constraint::Length(2),Constraint::Min(1),Constraint::Length(3)]).areas(frame.area());
            let working=rows.iter().skip(1).filter(|row|row.status()==Status::Working).count();
            frame.render_widget(Paragraph::new(format!("Codex agents — {working} working children")),title);
            frame.render_widget(Paragraph::new(render_rows(&rows,scroll,frame_index)).block(Block::bordered().title("Main + subagents")),body);
            frame.render_widget(Paragraph::new(format!("Recorded lifecycle • {warnings} diagnostics\n↑/↓ scroll • q quit • thread switching disabled")),footer);
        })?;
        frame_index = (frame_index + 1) % 4;
        if !event::poll(Duration::from_millis(150))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match key.code {
            KeyCode::Char('q') => return Ok(()),
            KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                return Ok(());
            }
            KeyCode::Down => scroll = (scroll + 1).min(rows.len().saturating_sub(1)),
            KeyCode::Up => scroll = scroll.saturating_sub(1),
            _ => {}
        }
    }
}

fn render_rows(rows: &[AgentRow], scroll: usize, frame: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if index != 0 && index <= scroll {
            continue;
        }
        let marker = if row.status() == Status::Working {
            ["| working", "/ working", "- working", "\\ working"]
                .get(frame)
                .copied()
                .unwrap_or("* working")
        } else {
            row.status().marker()
        };
        let color = match row.status() {
            Status::Working => Color::Cyan,
            Status::Waiting => Color::Yellow,
            Status::Failed | Status::Interrupted => Color::Red,
            _ => Color::Gray,
        };
        let stale = if row.freshness() == Freshness::Stale {
            " [stale]"
        } else {
            ""
        };
        let indentation = "  ".repeat(row.depth().min(20));
        lines.push(Line::from(vec![
            Span::styled(
                format!("{indentation}{marker} "),
                Style::default().fg(color),
            ),
            Span::raw(format!("{}{stale}", row.name())),
        ]));
        lines.push(Line::raw(format!(
            "{indentation}  Task: {}",
            row.assignment().unwrap_or("unavailable")
        )));
        lines.push(Line::raw(format!(
            "{indentation}  Last: {}",
            row.activity().unwrap_or("unavailable")
        )));
        lines.push(Line::raw(""));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{AgentTree, Event as AgentEvent, ThreadId};

    #[test]
    fn scrolling_children_keeps_main_visible() {
        let root = ThreadId::new("root".into());
        let mut tree = AgentTree::new(root.clone());
        for child in ["A", "B", "C"] {
            tree.apply(AgentEvent::Discover {
                thread: ThreadId::new(child.into()),
                parent: root.clone(),
                name: Some(format!("Worker-{child}")),
                assignment: None,
            })
            .unwrap();
        }
        let text = render_rows(&tree.rows(), 2, 0)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Main"));
        assert!(text.contains("Worker-C"));
        assert!(!text.contains("Worker-A"));
        assert!(!text.contains("Worker-B"));
    }
}
