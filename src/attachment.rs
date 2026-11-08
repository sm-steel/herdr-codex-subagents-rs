//! Automatic attachment of one monitor to each exactly identified Codex pane.

use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

use serde_json::{Value, json};

use crate::agent::ThreadId;
use crate::herdr::HerdrClient;
use crate::rollout::Monitor;

/// Reconcile monitors without focusing panes or sending terminal input.
///
/// # Errors
/// Returns an error for invalid host responses or unavailable registry storage.
pub fn sync(
    client: &HerdrClient,
    state_dir: &Path,
    connection: &str,
    sessions: &Path,
    codex_home: Option<&std::ffi::OsStr>,
) -> io::Result<Value> {
    let Some(_lock) = registry_lock(state_dir)? else {
        return Ok(json!({"busy":true}));
    };
    let path = state_dir.join("attachments.json");
    let mut registry = load(&path)?;
    registry
        .as_object_mut()
        .ok_or_else(|| io::Error::other("invalid attachment registry"))?
        .entry(connection.to_owned())
        .or_insert_with(|| json!({}));
    let response = client.query(&["agent".into(), "list".into()])?;
    let agents = response
        .pointer("/result/agents")
        .and_then(Value::as_array)
        .ok_or_else(|| io::Error::other("unsupported Herdr agent list"))?;
    let opened = 0;
    let mut existing = 0;
    let mut skipped = 0;
    for agent in agents.iter().filter(|a| a["agent"] == "codex").take(1000) {
        let (Some(pane), Some(terminal)) =
            (agent["pane_id"].as_str(), agent["terminal_id"].as_str())
        else {
            skipped += 1;
            continue;
        };
        let Ok(Some(root)) = client.codex_session(pane) else {
            skipped += 1;
            continue;
        };
        let records = registry
            .get(connection)
            .and_then(Value::as_object)
            .ok_or_else(|| io::Error::other("invalid server registry"))?;
        let previous = previous_record(client, records, pane, terminal, &root)?;
        if previous.as_ref().is_some_and(|record| {
            record["pending"] == true && record["source_terminal"] == terminal
        }) {
            skipped += 1;
            continue;
        }
        if let Some(record) = previous
            .as_ref()
            .filter(|record| record["root"] == root && record["source_terminal"] == terminal)
            && panel_exists(client, record)?
        {
            existing += 1;
            continue;
        }
        let mut monitor = Monitor::new(sessions.to_path_buf(), ThreadId::new(root.clone()));
        if monitor.discover().is_err() {
            skipped += 1;
            continue;
        }
        // Recheck the exact source after filesystem work, before changing its layout.
        if !source_matches(client, pane, terminal, &root)? {
            skipped += 1;
            continue;
        }
        if let Some(old) = previous.as_ref().filter(|old| old["panel"] != pane)
            && panel_exists(client, old)?
            && let Some(old_pane) = old.get("panel").and_then(Value::as_str)
            && client
                .query(&["pane".into(), "close".into(), old_pane.into()])
                .is_err()
        {
            skipped += 1;
            continue;
        }
        // Persist the attempt before mutation. An ambiguous host failure must not create duplicates.
        insert_record(
            &mut registry,
            connection,
            pane,
            json!({"root":root,"source_terminal":terminal,"pending":true}),
        )?;
        save(&path, &registry)?;
        let opened_panel = client.open_monitor(pane, &root, codex_home)?;
        insert_record(
            &mut registry,
            connection,
            pane,
            monitor_record(&opened_panel, &root, terminal)?,
        )?;
        save(&path, &registry)?;
        // Stop after each durable attachment; another invocation handles subsequent panes.
        return Ok(json!({"opened":opened + 1,"existing":existing,"skipped":skipped,"more":true}));
    }
    Ok(json!({"opened":opened,"existing":existing,"skipped":skipped,"more":false}))
}

fn panel_exists(client: &HerdrClient, record: &Value) -> io::Result<bool> {
    let (Some(pane), Some(terminal)) =
        (record["panel"].as_str(), record["panel_terminal"].as_str())
    else {
        return Ok(false);
    };
    match client.query(&["pane".into(), "get".into(), pane.into()]) {
        Ok(reply) => reply
            .pointer("/result/pane/terminal_id")
            .and_then(Value::as_str)
            .map(|id| id == terminal)
            .ok_or_else(|| io::Error::other("unsupported monitor identity response")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn previous_record(
    client: &HerdrClient,
    records: &serde_json::Map<String, Value>,
    pane: &str,
    terminal: &str,
    root: &str,
) -> io::Result<Option<Value>> {
    if let Some(record) = records.get(pane) {
        return Ok(Some(record.clone()));
    }
    for record in records
        .values()
        .filter(|record| record["source_terminal"] == terminal && record["root"] == root)
    {
        if panel_exists(client, record)? {
            return Ok(Some(record.clone()));
        }
    }
    Ok(None)
}
fn load(path: &Path) -> io::Result<Value> {
    match fs::File::open(path) {
        Ok(file) => {
            let mut bytes = Vec::new();
            file.take(1_048_577).read_to_end(&mut bytes)?;
            if bytes.len() > 1_048_576 {
                return Err(io::Error::other("attachment registry exceeds limit"));
            }
            serde_json::from_slice(&bytes).map_err(io::Error::other)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(json!({})),
        Err(error) => Err(error),
    }
}

fn save(path: &Path, registry: &Value) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("registry directory missing"))?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer(file.as_file_mut(), registry)?;
    file.flush()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

fn insert_record(
    registry: &mut Value,
    connection: &str,
    pane: &str,
    record: Value,
) -> io::Result<()> {
    registry
        .get_mut(connection)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| io::Error::other("invalid server registry"))?
        .insert(pane.to_owned(), record);
    Ok(())
}

fn monitor_record(reply: &Value, root: &str, source_terminal: &str) -> io::Result<Value> {
    let info = reply
        .pointer("/result/plugin_pane")
        .filter(|info| {
            info["plugin_id"] == "sm-steel.codex-subagents" && info["entrypoint"] == "monitor"
        })
        .ok_or_else(|| io::Error::other("unsupported plugin pane response"))?;
    let panel = info
        .pointer("/pane/pane_id")
        .and_then(Value::as_str)
        .ok_or_else(|| io::Error::other("monitor pane identity missing"))?;
    let panel_terminal = info
        .pointer("/pane/terminal_id")
        .and_then(Value::as_str)
        .ok_or_else(|| io::Error::other("monitor terminal identity missing"))?;
    Ok(
        json!({"root":root,"source_terminal":source_terminal,"panel":panel,"panel_terminal":panel_terminal}),
    )
}

fn source_matches(
    client: &HerdrClient,
    pane: &str,
    terminal: &str,
    root: &str,
) -> io::Result<bool> {
    let live = client.query(&["agent".into(), "get".into(), pane.into()])?;
    Ok(live
        .pointer("/result/agent/terminal_id")
        .and_then(Value::as_str)
        == Some(terminal)
        && client.codex_session(pane)?.as_deref() == Some(root))
}

fn registry_lock(state_dir: &Path) -> io::Result<Option<fs::File>> {
    fs::create_dir_all(state_dir)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(state_dir.join("attachment.lock"))?;
    match lock.try_lock() {
        Ok(()) => Ok(Some(lock)),
        Err(fs::TryLockError::WouldBlock) => Ok(None),
        Err(fs::TryLockError::Error(error)) => Err(error),
    }
}
