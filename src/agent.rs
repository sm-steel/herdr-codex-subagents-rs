//! Deterministic agent ancestry and explicit lifecycle observations.

use std::collections::BTreeMap;

const MAX_AGENTS: usize = 1_000;
const MAX_ACTIVITIES: usize = 200;
const MAX_TEXT_BYTES: usize = 4_096;

/// An exact upstream thread identifier, independent of its display name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ThreadId(String);

impl ThreadId {
    /// Wrap an identifier obtained from an upstream identity field.
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// Exact upstream identifier, never a display name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Observed lifecycle state; silence is never a terminal event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Status {
    /// No authoritative lifecycle information is available.
    Unknown,
    /// The agent has an active turn.
    Working,
    /// The agent explicitly waits for input, approval, or children.
    Waiting,
    /// Explicitly idle, with no active turn.
    Idle,
    /// The observed turn completed successfully.
    Completed,
    /// The observed turn failed.
    Failed,
    /// The observed turn was interrupted.
    Interrupted,
    /// The thread was explicitly closed.
    Closed,
}

/// Whether the source remains available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    /// The source is available; lack of new output is normal.
    Fresh,
    /// Source observation failed; the retained state is historical.
    Stale,
}

/// A normalized event produced by the versioned Codex adapter.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Event {
    /// Child metadata with exact parent identity.
    Discover {
        /// Child identifier.
        thread: ThreadId,
        /// Parent identifier.
        parent: ThreadId,
        /// Optional human-readable name.
        name: Option<String>,
        /// Optional original delegated task.
        assignment: Option<String>,
    },
    /// An explicit lifecycle observation scoped to a turn.
    Lifecycle {
        /// Thread identifier.
        thread: ThreadId,
        /// Opaque turn identity; late terminal events must not end a newer turn.
        turn: String,
        /// Observed state.
        status: Status,
    },
    /// Latest observable message or tool activity, with a stable event identity.
    Activity {
        /// Thread identifier.
        thread: ThreadId,
        /// Source event identity, used for deduplication.
        event_id: String,
        /// Human-readable activity; sanitize at the input boundary.
        text: String,
    },
    /// Source availability changed without evidence of lifecycle completion.
    Source {
        /// Thread identifier.
        thread: ThreadId,
        /// New source freshness.
        freshness: Freshness,
    },
    /// A user task or delegated assignment observed for this thread.
    Assignment {
        /// Exact thread identity.
        thread: ThreadId,
        /// Recorded task text, separate from activity.
        text: String,
    },
}

/// A display row with encapsulated fields.
#[derive(Debug, Clone)]
pub struct AgentRow {
    thread: ThreadId,
    depth: usize,
    status: Status,
    freshness: Freshness,
    name: String,
    assignment: Option<String>,
    activity: Option<String>,
    activity_count: usize,
}

impl AgentRow {
    /// Exact thread identity.
    #[must_use]
    pub const fn thread(&self) -> &ThreadId {
        &self.thread
    }
    /// Indentation level from the root.
    #[must_use]
    pub const fn depth(&self) -> usize {
        self.depth
    }
    /// Last explicitly observed lifecycle state.
    #[must_use]
    pub const fn status(&self) -> Status {
        self.status
    }
    /// Availability of the observation source.
    #[must_use]
    pub const fn freshness(&self) -> Freshness {
        self.freshness
    }
    /// Display name, never used as identity.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Recorded delegated task if available.
    #[must_use]
    pub fn assignment(&self) -> Option<&str> {
        self.assignment.as_deref()
    }
    /// Most recently observed activity if available.
    #[must_use]
    pub fn activity(&self) -> Option<&str> {
        self.activity.as_deref()
    }
    /// Number of retained distinct activities, bounded by the retention limit.
    #[must_use]
    pub const fn activity_count(&self) -> usize {
        self.activity_count
    }
}

/// Invalid ancestry cannot enter the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidAncestry;

/// Domain state; all I/O and time remain in adapters.
pub struct AgentTree {
    root: ThreadId,
    nodes: BTreeMap<ThreadId, Node>,
    order: Vec<ThreadId>,
}

struct Node {
    row: AgentRow,
    parent: Option<ThreadId>,
    turn: Option<String>,
    activities: Vec<(String, String)>,
}

impl Node {
    fn new(thread: ThreadId, name: String) -> Self {
        Self {
            row: AgentRow {
                thread,
                depth: 0,
                status: Status::Unknown,
                freshness: Freshness::Fresh,
                name,
                assignment: None,
                activity: None,
                activity_count: 0,
            },
            parent: None,
            turn: None,
            activities: Vec::new(),
        }
    }
}

impl AgentTree {
    /// Construct a tree rooted at an exact session ID.
    #[must_use]
    pub fn new(root: ThreadId) -> Self {
        let node = Node::new(root.clone(), "Main".into());
        Self {
            nodes: BTreeMap::from([(root.clone(), node)]),
            order: vec![root.clone()],
            root,
        }
    }

    /// Apply one observation.
    ///
    /// # Errors
    /// Returns an error for cycles, self-parenting, and attempts to reparent root.
    pub fn apply(&mut self, event: Event) -> Result<(), InvalidAncestry> {
        match event {
            Event::Discover {
                thread,
                parent,
                name,
                assignment,
            } => {
                self.check_ancestry(&thread, &parent)?;
                if let Some(node) = self.ensure(&thread) {
                    node.parent = Some(parent);
                    if let Some(name) = name {
                        node.row.name = bounded_text(&name);
                    }
                    if let Some(assignment) = assignment {
                        node.row.assignment = Some(bounded_text(&assignment));
                    }
                }
            }
            Event::Lifecycle {
                thread,
                turn,
                status,
            } => {
                if let Some(node) = self.ensure(&thread)
                    && (status == Status::Working
                        || node.turn.as_ref().is_none_or(|active| active == &turn))
                {
                    node.turn = Some(turn);
                    node.row.status = status;
                }
            }
            Event::Activity {
                thread,
                event_id,
                text,
            } => {
                if let Some(node) = self.ensure(&thread)
                    && !node.activities.iter().any(|(seen, _)| seen == &event_id)
                {
                    let text = bounded_text(&text);
                    node.activities.push((event_id, text.clone()));
                    if node.activities.len() > MAX_ACTIVITIES {
                        node.activities.remove(0);
                    }
                    node.row.activity = Some(text);
                    node.row.activity_count = node.activities.len();
                }
            }
            Event::Source { thread, freshness } => {
                if let Some(node) = self.ensure(&thread) {
                    node.row.freshness = freshness;
                }
            }
            Event::Assignment { thread, text } => {
                if let Some(node) = self.ensure(&thread) {
                    node.row.assignment = Some(bounded_text(&text));
                }
            }
        }
        Ok(())
    }

    /// Return main first, then known descendants in stable ancestry order.
    #[must_use]
    pub fn rows(&self) -> Vec<AgentRow> {
        let mut rows = Vec::new();
        self.append_rows(&self.root, 0, &mut rows);
        rows
    }

    /// Number of working descendants, independent of the root's own status.
    #[must_use]
    pub fn working_descendants(&self) -> usize {
        self.rows()
            .iter()
            .skip(1)
            .filter(|row| row.status == Status::Working)
            .count()
    }

    fn ensure(&mut self, thread: &ThreadId) -> Option<&mut Node> {
        if !self.nodes.contains_key(thread) {
            if self.nodes.len() >= MAX_AGENTS {
                return None;
            }
            self.order.push(thread.clone());
            self.nodes.insert(
                thread.clone(),
                Node::new(thread.clone(), thread.as_str().into()),
            );
        }
        self.nodes.get_mut(thread)
    }

    fn check_ancestry(&self, thread: &ThreadId, parent: &ThreadId) -> Result<(), InvalidAncestry> {
        if thread == &self.root || thread == parent {
            return Err(InvalidAncestry);
        }
        let mut cursor = Some(parent);
        while let Some(current) = cursor {
            if current == thread {
                return Err(InvalidAncestry);
            }
            cursor = self
                .nodes
                .get(current)
                .and_then(|node| node.parent.as_ref());
        }
        Ok(())
    }

    fn append_rows(&self, thread: &ThreadId, depth: usize, rows: &mut Vec<AgentRow>) {
        let Some(node) = self.nodes.get(thread) else {
            return;
        };
        let mut row = node.row.clone();
        row.depth = depth;
        rows.push(row);
        for child in &self.order {
            if self
                .nodes
                .get(child)
                .is_some_and(|node| node.parent.as_ref() == Some(thread))
            {
                self.append_rows(child, depth + 1, rows);
            }
        }
    }
}

fn bounded_text(text: &str) -> String {
    let mut end = text.len().min(MAX_TEXT_BYTES);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.get(..end).unwrap_or_default().into()
}

impl Status {
    /// Text-mode status marker; color and animation are optional enhancements.
    #[must_use]
    pub const fn marker(self) -> &'static str {
        match self {
            Self::Unknown => "? unknown",
            Self::Working => "* working",
            Self::Waiting => "~ waiting",
            Self::Idle => "- idle",
            Self::Completed => "+ completed",
            Self::Failed => "! failed",
            Self::Interrupted => "! interrupted",
            Self::Closed => "x closed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: &str) -> ThreadId {
        ThreadId(value.into())
    }
    fn tree() -> AgentTree {
        AgentTree::new(id("root"))
    }
    fn discover(tree: &mut AgentTree, child: &str, parent: &str, name: &str) {
        tree.apply(Event::Discover {
            thread: id(child),
            parent: id(parent),
            name: Some(name.into()),
            assignment: Some(format!("Task for {child}")),
        })
        .unwrap();
    }
    fn state(tree: &mut AgentTree, thread: &str, turn: &str, status: Status) {
        tree.apply(Event::Lifecycle {
            thread: id(thread),
            turn: turn.into(),
            status,
        })
        .unwrap();
    }
    fn activity(tree: &mut AgentTree, thread: &str, event_id: &str, text: &str) {
        tree.apply(Event::Activity {
            thread: id(thread),
            event_id: event_id.into(),
            text: text.into(),
        })
        .unwrap();
    }

    #[test]
    fn main_is_first_even_when_child_is_discovered_first() {
        let mut tree = tree();
        discover(&mut tree, "child", "root", "Worker");
        let rows = tree.rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].thread, id("root"));
        assert_eq!(rows[1].thread, id("child"));
    }
    #[test]
    fn every_lifecycle_has_a_distinct_text_marker() {
        let cases = [
            (Status::Working, "* working"),
            (Status::Waiting, "~ waiting"),
            (Status::Idle, "- idle"),
            (Status::Completed, "+ completed"),
            (Status::Failed, "! failed"),
            (Status::Interrupted, "! interrupted"),
            (Status::Closed, "x closed"),
            (Status::Unknown, "? unknown"),
        ];
        for (status, marker) in cases {
            assert_eq!(status.marker(), marker);
        }
    }
    #[test]
    fn both_main_and_child_support_working_state() {
        let mut tree = tree();
        discover(&mut tree, "child", "root", "Worker");
        state(&mut tree, "root", "r1", Status::Working);
        state(&mut tree, "child", "c1", Status::Working);
        let rows = tree.rows();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row.status == Status::Working));
    }
    #[test]
    fn idle_main_stays_idle_with_working_children() {
        let mut tree = tree();
        discover(&mut tree, "child", "root", "Worker");
        state(&mut tree, "root", "r1", Status::Idle);
        state(&mut tree, "child", "c1", Status::Working);
        assert_eq!(tree.rows()[0].status, Status::Idle);
        assert_eq!(tree.working_descendants(), 1);
    }
    #[test]
    fn explicit_wait_does_not_mean_completed() {
        let mut tree = tree();
        state(&mut tree, "root", "r1", Status::Working);
        state(&mut tree, "root", "r1", Status::Waiting);
        assert_eq!(tree.rows()[0].status, Status::Waiting);
    }
    #[test]
    fn explicit_resume_changes_waiting_to_working() {
        let mut tree = tree();
        state(&mut tree, "root", "r1", Status::Waiting);
        state(&mut tree, "root", "r1", Status::Working);
        assert_eq!(tree.rows()[0].status, Status::Working);
    }
    #[test]
    fn terminal_outcomes_remain_distinct() {
        for status in [
            Status::Completed,
            Status::Failed,
            Status::Interrupted,
            Status::Closed,
        ] {
            let mut tree = tree();
            state(&mut tree, "root", "r1", status);
            assert_eq!(tree.rows()[0].status, status);
        }
    }
    #[test]
    fn source_loss_keeps_last_status_but_marks_stale() {
        let mut tree = tree();
        state(&mut tree, "root", "r1", Status::Working);
        tree.apply(Event::Source {
            thread: id("root"),
            freshness: Freshness::Stale,
        })
        .unwrap();
        let rows = tree.rows();
        assert_eq!(rows[0].status, Status::Working);
        assert_eq!(rows[0].freshness, Freshness::Stale);
    }
    #[test]
    fn recovered_source_restores_freshness_without_inventing_completion() {
        let mut tree = tree();
        state(&mut tree, "root", "r1", Status::Working);
        for freshness in [Freshness::Stale, Freshness::Fresh] {
            tree.apply(Event::Source {
                thread: id("root"),
                freshness,
            })
            .unwrap();
        }
        assert_eq!(tree.rows()[0].freshness, Freshness::Fresh);
        assert_eq!(tree.rows()[0].status, Status::Working);
    }
    #[test]
    fn duplicate_names_do_not_merge_threads() {
        let mut tree = tree();
        discover(&mut tree, "a", "root", "Worker");
        discover(&mut tree, "b", "root", "Worker");
        let rows = tree.rows();
        assert_eq!(rows.len(), 3);
        assert_ne!(rows[1].thread, rows[2].thread);
    }
    #[test]
    fn rediscovery_updates_metadata_without_reordering_or_duplicates() {
        let mut tree = tree();
        discover(&mut tree, "b", "root", "B");
        discover(&mut tree, "a", "root", "A");
        discover(&mut tree, "b", "root", "Renamed");
        let rows = tree.rows();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1].thread, id("b"));
        assert_eq!(rows[1].name, "Renamed");
        assert_eq!(rows[2].thread, id("a"));
    }
    #[test]
    fn completed_children_remain_visible() {
        let mut tree = tree();
        discover(&mut tree, "child", "root", "Worker");
        state(&mut tree, "child", "c1", Status::Completed);
        let rows = tree.rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].status, Status::Completed);
    }
    #[test]
    fn unrelated_parent_is_not_displayed_as_our_child() {
        let mut tree = tree();
        discover(&mut tree, "other-child", "other-root", "Other");
        assert_eq!(tree.rows().len(), 1);
    }
    #[test]
    fn late_parent_reveals_grandchild_with_correct_depth() {
        let mut tree = tree();
        discover(&mut tree, "grandchild", "child", "Grandchild");
        assert_eq!(tree.rows().len(), 1);
        discover(&mut tree, "child", "root", "Child");
        let rows = tree.rows();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1].thread, id("child"));
        assert_eq!(rows[1].depth, 1);
        assert_eq!(rows[2].thread, id("grandchild"));
        assert_eq!(rows[2].depth, 2);
    }
    #[test]
    fn rejects_self_parent_and_root_reparenting() {
        for (thread, parent) in [("child", "child"), ("root", "child")] {
            let mut tree = tree();
            assert!(
                tree.apply(Event::Discover {
                    thread: id(thread),
                    parent: id(parent),
                    name: None,
                    assignment: None
                })
                .is_err()
            );
        }
    }
    #[test]
    fn assignment_and_activity_are_separate() {
        let mut tree = tree();
        discover(&mut tree, "child", "root", "Worker");
        activity(&mut tree, "child", "e1", "Reading auth.rs");
        let rows = tree.rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].assignment.as_deref(), Some("Task for child"));
        assert_eq!(rows[1].activity.as_deref(), Some("Reading auth.rs"));
    }
    #[test]
    fn repeated_activity_is_deduplicated() {
        let mut tree = tree();
        activity(&mut tree, "root", "e1", "Reading auth.rs");
        activity(&mut tree, "root", "e1", "Reading auth.rs");
        assert_eq!(tree.rows()[0].activity_count, 1);
    }
    #[test]
    fn new_turn_restarts_a_completed_agent() {
        let mut tree = tree();
        state(&mut tree, "root", "r1", Status::Completed);
        state(&mut tree, "root", "r2", Status::Working);
        assert_eq!(tree.rows()[0].status, Status::Working);
    }
    #[test]
    fn late_completion_cannot_end_a_newer_turn() {
        let mut tree = tree();
        state(&mut tree, "root", "r1", Status::Working);
        state(&mut tree, "root", "r2", Status::Working);
        state(&mut tree, "root", "r1", Status::Completed);
        assert_eq!(tree.rows()[0].status, Status::Working);
    }
    #[test]
    fn activity_does_not_invent_working_or_completed_status() {
        let mut tree = tree();
        activity(&mut tree, "root", "e1", "Last message");
        assert_eq!(tree.rows()[0].status, Status::Unknown);
        assert_eq!(tree.rows()[0].activity.as_deref(), Some("Last message"));
    }
    #[test]
    fn main_assignment_updates_without_reparenting() {
        let mut tree = tree();
        tree.apply(Event::Assignment {
            thread: id("root"),
            text: "Review auth".into(),
        })
        .unwrap();
        assert_eq!(tree.rows()[0].assignment.as_deref(), Some("Review auth"));
        assert_eq!(tree.rows()[0].thread, id("root"));
    }
    #[test]
    fn latest_assignment_does_not_overwrite_activity() {
        let mut tree = tree();
        activity(&mut tree, "root", "e1", "Reading file");
        tree.apply(Event::Assignment {
            thread: id("root"),
            text: "New task".into(),
        })
        .unwrap();
        assert_eq!(tree.rows()[0].assignment.as_deref(), Some("New task"));
        assert_eq!(tree.rows()[0].activity.as_deref(), Some("Reading file"));
    }
}
