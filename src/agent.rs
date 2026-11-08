//! Agent-tree and lifecycle contract; reducer implementation awaits test review.

/// An exact upstream thread identifier, independent of its display name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ThreadId(String);

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

/// Invalid ancestry cannot enter the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidAncestry;

/// Domain state. This stub intentionally ignores events until tests are approved.
pub struct AgentTree {
    root: ThreadId,
}

impl AgentTree {
    /// Construct a tree rooted at an exact session ID.
    #[must_use]
    pub const fn new(root: ThreadId) -> Self {
        Self { root }
    }

    /// Apply one observation.
    ///
    /// # Errors
    /// Returns an error for cycles, self-parenting, and attempts to reparent root.
    pub fn apply(&mut self, _event: Event) -> Result<(), InvalidAncestry> {
        Ok(())
    }

    /// Return main first, then known descendants in stable ancestry order.
    #[must_use]
    pub fn rows(&self) -> Vec<AgentRow> {
        vec![AgentRow {
            thread: self.root.clone(),
            depth: 0,
            status: Status::Unknown,
            freshness: Freshness::Fresh,
            name: "Main".into(),
            assignment: None,
            activity: None,
            activity_count: 0,
        }]
    }

    /// Number of working descendants, independent of the root's own status.
    #[must_use]
    pub const fn working_descendants(&self) -> usize {
        0
    }
}

impl Status {
    /// Text-mode status marker; color and animation are optional enhancements.
    #[must_use]
    pub const fn marker(self) -> &'static str {
        "? unknown"
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
}
