//! Read-only, bounded observation of local Codex rollout files.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;

use crate::agent::{AgentRow, AgentTree, Event, Freshness, Status, ThreadId};
use crate::record_buffer::{Record, RecordBuffer};
use crate::text::display_text;

const RECORD_LIMIT: usize = 1_048_576;
const READ_BUDGET: u64 = 4_194_304;
const MAX_FILES: usize = 10_000;

/// A local observer, pinned to an exact root thread ID.
pub struct Monitor {
    sessions_dir: PathBuf,
    root: ThreadId,
    tree: AgentTree,
    files: BTreeMap<PathBuf, Tail>,
    indexed: BTreeSet<PathBuf>,
    candidates: BTreeMap<PathBuf, Metadata>,
    warnings: usize,
}

struct Metadata {
    thread: ThreadId,
    parent: Option<ThreadId>,
    name: Option<String>,
}
struct Tail {
    thread: ThreadId,
    offset: u64,
    modified: Option<SystemTime>,
    buffer: RecordBuffer,
    context: DecodeContext,
}

#[derive(Default)]
struct DecodeContext {
    pending: BTreeMap<String, String>,
    turn: Option<String>,
}

impl Monitor {
    /// Observe only this root and its descendants under a session-log directory.
    #[must_use]
    pub fn new(sessions_dir: PathBuf, root: ThreadId) -> Self {
        Self {
            sessions_dir,
            tree: AgentTree::new(root.clone()),
            root,
            files: BTreeMap::new(),
            indexed: BTreeSet::new(),
            candidates: BTreeMap::new(),
            warnings: 0,
        }
    }

    /// Discover new rollouts; call less often than `poll` for large histories.
    ///
    /// # Errors
    /// Returns an error if the session directory cannot be inspected.
    pub fn discover(&mut self) -> io::Result<()> {
        let _span =
            tracing::debug_span!("rollout_discovery", known_files = self.indexed.len()).entered();
        let mut paths = Vec::new();
        collect_paths(&self.sessions_dir, &mut paths)?;
        for path in paths {
            if self.indexed.contains(&path) {
                continue;
            }
            match read_metadata(&path) {
                Ok(Some(metadata)) => {
                    self.indexed.insert(path.clone());
                    self.candidates.insert(path, metadata);
                }
                Ok(None) => {}
                Err(_) => {
                    self.warnings += 1;
                }
            }
        }
        let mut known = BTreeSet::from([self.root.clone()]);
        loop {
            let previous = known.len();
            for metadata in self.candidates.values() {
                if metadata
                    .parent
                    .as_ref()
                    .is_some_and(|parent| known.contains(parent))
                {
                    known.insert(metadata.thread.clone());
                }
            }
            if previous == known.len() {
                break;
            }
        }
        for (path, metadata) in &self.candidates {
            if !known.contains(&metadata.thread) || self.files.contains_key(path) {
                continue;
            }
            if let Some(parent) = &metadata.parent {
                let _ = self.tree.apply(Event::Discover {
                    thread: metadata.thread.clone(),
                    parent: parent.clone(),
                    name: metadata.name.clone(),
                    assignment: None,
                });
            }
            self.files.insert(
                path.clone(),
                Tail {
                    thread: metadata.thread.clone(),
                    offset: 0,
                    modified: None,
                    buffer: RecordBuffer::new(RECORD_LIMIT),
                    context: DecodeContext::default(),
                },
            );
        }
        if !self.files.values().any(|tail| tail.thread == self.root) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "root thread rollout not found",
            ));
        }
        Ok(())
    }

    /// Incrementally read bounded new bytes and return the current tree.
    #[must_use]
    pub fn poll(&mut self) -> Vec<AgentRow> {
        let _span = tracing::debug_span!("rollout_poll", sources = self.files.len()).entered();
        let mut budget = READ_BUDGET;
        for (path, tail) in &mut self.files {
            if budget == 0 {
                break;
            }
            let Ok(records) = tail.read(path, &mut budget) else {
                self.warnings += 1;
                let _ = self.tree.apply(Event::Source {
                    thread: tail.thread.clone(),
                    freshness: Freshness::Stale,
                });
                continue;
            };
            let _ = self.tree.apply(Event::Source {
                thread: tail.thread.clone(),
                freshness: Freshness::Fresh,
            });
            self.warnings +=
                apply_records(&mut self.tree, &tail.thread, &mut tail.context, records);
        }
        self.tree.rows()
    }

    /// Count of malformed/unavailable observations; never includes raw data.
    #[must_use]
    pub const fn warnings(&self) -> usize {
        self.warnings
    }
}

fn apply_records(
    tree: &mut AgentTree,
    thread: &ThreadId,
    context: &mut DecodeContext,
    records: Vec<(u64, Record)>,
) -> usize {
    let mut warnings = 0;
    for (offset, record) in records {
        let Record::Line(bytes) = record else {
            warnings += 1;
            continue;
        };
        let Ok(record) = serde_json::from_slice::<Value>(&bytes) else {
            warnings += 1;
            continue;
        };
        for event in decode(&record, thread, offset, context) {
            if tree.apply(event).is_err() {
                warnings += 1;
            }
        }
    }
    warnings
}

impl Tail {
    fn read(&mut self, path: &Path, budget: &mut u64) -> io::Result<Vec<(u64, Record)>> {
        let mut file = File::open(path)?;
        let metadata = file.metadata()?;
        let modified = metadata.modified().ok();
        if metadata.len() < self.offset
            || (metadata.len() == self.offset && modified != self.modified)
        {
            self.offset = 0;
            self.buffer.reset();
            self.context = DecodeContext::default();
        }
        file.seek(SeekFrom::Start(self.offset))?;
        let mut bytes = Vec::new();
        file.take(*budget).read_to_end(&mut bytes)?;
        *budget = budget.saturating_sub(bytes.len() as u64);
        let start = self.offset;
        self.offset += bytes.len() as u64;
        self.modified = modified;
        // Identity is the byte offset of the completed record, not its read chunk.
        let mut records = Vec::new();
        for (index, byte) in bytes.iter().enumerate() {
            for record in self.buffer.push(std::slice::from_ref(byte)) {
                records.push((start + index as u64, record));
            }
        }
        Ok(records)
    }
}

fn collect_paths(directory: &Path, paths: &mut Vec<PathBuf>) -> io::Result<()> {
    if paths.len() >= MAX_FILES {
        return Ok(());
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            collect_paths(&entry.path(), paths)?;
        } else if kind.is_file() && entry.path().extension().is_some_and(|ext| ext == "jsonl") {
            paths.push(entry.path());
        }
        if paths.len() >= MAX_FILES {
            break;
        }
    }
    paths.sort();
    Ok(())
}

fn read_metadata(path: &Path) -> io::Result<Option<Metadata>> {
    let mut bytes = Vec::new();
    BufReader::new(File::open(path)?.take(RECORD_LIMIT as u64 + 1))
        .read_until(b'\n', &mut bytes)?;
    if bytes.len() > RECORD_LIMIT || bytes.last() != Some(&b'\n') {
        return Ok(None);
    }
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
        return Ok(None);
    };
    if text(&value, "/type") != Some("session_meta") {
        return Ok(None);
    }
    let Some(thread) = text(&value, "/payload/id") else {
        return Ok(None);
    };
    if !valid_identifier(thread) {
        return Ok(None);
    }
    let parent = text(&value, "/payload/parent_thread_id").or_else(|| {
        text(
            &value,
            "/payload/source/subagent/thread_spawn/parent_thread_id",
        )
    });
    let name =
        text(&value, "/payload/agent_path").or_else(|| text(&value, "/payload/agent_nickname"));
    Ok(Some(Metadata {
        thread: ThreadId::new(thread.into()),
        parent: parent
            .filter(|value| valid_identifier(value))
            .map(|value| ThreadId::new(value.into())),
        name: name.map(|value| display_text(value, 256)),
    }))
}

fn text<'a>(value: &'a Value, path: &str) -> Option<&'a str> {
    value.pointer(path)?.as_str()
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn decode(
    record: &Value,
    thread: &ThreadId,
    offset: u64,
    context: &mut DecodeContext,
) -> Vec<Event> {
    if text(record, "/type") == Some("response_item") {
        if text(record, "/payload/type") == Some("function_call")
            && text(record, "/payload/name") == Some("spawn_agent")
            && let (Some(call_id), Some(arguments)) = (
                text(record, "/payload/call_id"),
                text(record, "/payload/arguments"),
            )
            && let Ok(arguments) = serde_json::from_str::<Value>(arguments)
            && let Some(message) = text(&arguments, "/message")
            && context.pending.len() < 1000
        {
            context
                .pending
                .insert(display_text(call_id, 256), display_text(message, 4096));
        }
        return Vec::new();
    }
    if text(record, "/type") != Some("event_msg") {
        return Vec::new();
    }
    let Some(payload) = record.get("payload") else {
        return Vec::new();
    };
    let kind = text(payload, "/type").unwrap_or_default();
    let turn = text(payload, "/turn_id").or_else(|| text(payload, "/turn_attribution/turn_id"));
    if let Some(turn) = turn {
        context.turn = Some(display_text(turn, 256));
    }
    let status = match kind {
        "task_started" => Some(Status::Working),
        "task_complete" => Some(Status::Completed),
        "turn_aborted" | "task_interrupted" => Some(Status::Interrupted),
        "task_failed" => Some(Status::Failed),
        _ => None,
    };
    let mut events = Vec::new();
    if text(payload, "/item/type") == Some("SubAgentActivity")
        && text(payload, "/item/kind") == Some("started")
        && let Some(child) =
            text(payload, "/item/agent_thread_id").filter(|value| valid_identifier(value))
    {
        let child = ThreadId::new(display_text(child, 128));
        events.push(Event::Discover {
            thread: child.clone(),
            parent: thread.clone(),
            name: text(payload, "/item/agent_path").map(|name| display_text(name, 256)),
            assignment: None,
        });
        if let Some(call_id) = text(payload, "/item/id")
            && let Some(assignment) = context.pending.remove(call_id)
        {
            events.push(Event::Assignment {
                thread: child,
                text: assignment,
            });
        }
    }
    if text(payload, "/item/type") == Some("CollabAgentToolCall")
        && text(payload, "/item/tool") == Some("wait")
        && let Some(turn) = &context.turn
    {
        events.push(Event::Lifecycle {
            thread: thread.clone(),
            turn: turn.clone(),
            status: if kind == "item_started" {
                Status::Waiting
            } else {
                Status::Working
            },
        });
    }
    if let (Some(status), Some(turn)) = (status, turn) {
        events.push(Event::Lifecycle {
            thread: thread.clone(),
            turn: turn.into(),
            status,
        });
    }
    let message = match kind {
        "agent_message" | "user_message" => text(payload, "/message"),
        "task_complete" => text(payload, "/last_agent_message"),
        "item_started" | "item_completed" => item_text(payload),
        _ => None,
    };
    if let Some(message) = message {
        let message = display_text(message, 4096);
        if kind == "user_message" || text(payload, "/item/type") == Some("UserMessage") {
            events.push(Event::Assignment {
                thread: thread.clone(),
                text: message,
            });
        } else {
            events.push(Event::Activity {
                thread: thread.clone(),
                event_id: format!("record:{offset}"),
                text: message,
            });
        }
    }
    events
}

fn item_text(payload: &Value) -> Option<&str> {
    match text(payload, "/item/type")? {
        "AgentMessage" | "UserMessage" => text(payload, "/item/content/0/text"),
        "CommandExecution" => Some("Running a command"),
        "FileChange" => Some("Changing files"),
        "McpToolCall" => Some("Calling an MCP tool"),
        "WebSearch" => Some("Searching the web"),
        _ => None,
    }
}
