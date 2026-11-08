use std::fs::{self, OpenOptions};
use std::io::Write;

use herdr_codex_subagents_rs::agent::{Status, ThreadId};
use herdr_codex_subagents_rs::rollout::Monitor;
use tempfile::TempDir;

fn metadata(id: &str, parent: Option<&str>) -> String {
    format!(
        "{}\n",
        serde_json::json!({"type":"session_meta", "payload":{
        "id":id,"parent_thread_id":parent,"agent_nickname":"Worker"}})
    )
}
fn event(kind: &str, turn: &str) -> String {
    format!(
        "{}\n",
        serde_json::json!({"type":"event_msg", "payload":{"type":kind,"turn_id":turn}})
    )
}
fn monitor(dir: &TempDir) -> Monitor {
    Monitor::new(dir.path().into(), ThreadId::new("root".into()))
}

#[test]
fn exact_root_excludes_same_directory_unrelated_sessions() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("root.jsonl"),
        metadata("root", None) + &event("task_started", "r1"),
    )
    .unwrap();
    fs::write(dir.path().join("other.jsonl"), metadata("other", None)).unwrap();
    fs::write(
        dir.path().join("child.jsonl"),
        metadata("child", Some("root")) + &event("task_complete", "c1"),
    )
    .unwrap();
    let mut reader = monitor(&dir);
    reader.discover().unwrap();
    let rows = reader.poll();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].thread().as_str(), "root");
    assert_eq!(rows[0].status(), Status::Working);
    assert_eq!(rows[1].status(), Status::Completed);
}
#[test]
fn reads_appends_without_replaying_previous_activity() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("root.jsonl");
    fs::write(&path, metadata("root", None) + &event("task_started", "r1")).unwrap();
    let mut reader = monitor(&dir);
    reader.discover().unwrap();
    assert_eq!(reader.poll()[0].status(), Status::Working);
    OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(event("task_complete", "r1").as_bytes())
        .unwrap();
    assert_eq!(reader.poll()[0].status(), Status::Completed);
    assert_eq!(reader.poll()[0].activity_count(), 0);
}
#[test]
fn malformed_record_does_not_hide_later_completion() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("root.jsonl"),
        metadata("root", None) + "not json\n" + &event("task_complete", "r1"),
    )
    .unwrap();
    let mut reader = monitor(&dir);
    reader.discover().unwrap();
    assert_eq!(reader.poll()[0].status(), Status::Completed);
    assert_eq!(reader.warnings(), 1);
}
#[test]
fn child_created_later_is_discovered_without_restarting_monitor() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("root.jsonl"), metadata("root", None)).unwrap();
    let mut reader = monitor(&dir);
    reader.discover().unwrap();
    assert_eq!(reader.poll().len(), 1);
    fs::write(
        dir.path().join("child.jsonl"),
        metadata("child", Some("root")),
    )
    .unwrap();
    reader.discover().unwrap();
    assert_eq!(reader.poll().len(), 2);
}
#[test]
fn missing_root_returns_error_instead_of_choosing_newest() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("other.jsonl"), metadata("other", None)).unwrap();
    assert!(monitor(&dir).discover().is_err());
}
#[test]
fn partial_appended_json_is_not_treated_as_malformed() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("root.jsonl");
    let completion = event("task_complete", "r1");
    let split = completion.len() / 2;
    fs::write(&path, metadata("root", None) + &completion[..split]).unwrap();
    let mut reader = monitor(&dir);
    reader.discover().unwrap();
    assert_eq!(reader.poll()[0].status(), Status::Unknown);
    assert_eq!(reader.warnings(), 0);
    OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(&completion.as_bytes()[split..])
        .unwrap();
    assert_eq!(reader.poll()[0].status(), Status::Completed);
}

#[test]
fn v2_assignment_uses_exact_call_id_not_spawn_order_or_name() {
    let dir = TempDir::new().unwrap();
    let records = [
        serde_json::json!({"type":"response_item","payload":{"type":"function_call","name":"spawn_agent","call_id":"a","arguments":"{\"message\":\"Task A\"}"}}),
        serde_json::json!({"type":"response_item","payload":{"type":"function_call","name":"spawn_agent","call_id":"b","arguments":"{\"message\":\"Task B\"}"}}),
        serde_json::json!({"type":"event_msg","payload":{"type":"item_completed","item":{"type":"SubAgentActivity","kind":"started","id":"b","agent_thread_id":"child-b","agent_path":"/root/same"}}}),
        serde_json::json!({"type":"event_msg","payload":{"type":"item_completed","item":{"type":"SubAgentActivity","kind":"started","id":"a","agent_thread_id":"child-a","agent_path":"/root/same"}}}),
    ];
    let mut content = metadata("root", None);
    for record in records {
        content.push_str(&record.to_string());
        content.push('\n');
    }
    fs::write(dir.path().join("root.jsonl"), content).unwrap();
    let mut reader = monitor(&dir);
    reader.discover().unwrap();
    let rows = reader.poll();
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows.iter()
            .find(|row| row.thread().as_str() == "child-a")
            .unwrap()
            .assignment(),
        Some("Task A")
    );
    assert_eq!(
        rows.iter()
            .find(|row| row.thread().as_str() == "child-b")
            .unwrap()
            .assignment(),
        Some("Task B")
    );
}

#[test]
fn v2_explicit_wait_has_waiting_state_until_wait_completes() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("root.jsonl");
    let wait = |kind: &str| {
        format!(
            "{}\n",
            serde_json::json!({"type":"event_msg","payload":{
        "type":kind,"turn_id":"r1","item":{"type":"CollabAgentToolCall","tool":"wait"}}})
        )
    };
    fs::write(
        &path,
        metadata("root", None) + &event("task_started", "r1") + &wait("item_started"),
    )
    .unwrap();
    let mut reader = monitor(&dir);
    reader.discover().unwrap();
    assert_eq!(reader.poll()[0].status(), Status::Waiting);
    OpenOptions::new()
        .append(true)
        .open(path)
        .unwrap()
        .write_all(wait("item_completed").as_bytes())
        .unwrap();
    assert_eq!(reader.poll()[0].status(), Status::Working);
}

#[test]
fn larger_replacement_replays_new_turn_instead_of_skipping_prefix() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("root.jsonl");
    fs::write(&path, metadata("root", None) + &event("task_started", "r1")).unwrap();
    let mut reader = monitor(&dir);
    reader.discover().unwrap();
    assert_eq!(reader.poll()[0].status(), Status::Working);
    let replacement =
        metadata("root", None) + &event("task_started", "r2") + &event("task_complete", "r2");
    let new_path = dir.path().join("replacement.tmp");
    fs::write(&new_path, replacement).unwrap();
    fs::remove_file(&path).unwrap();
    fs::rename(new_path, &path).unwrap();
    assert_eq!(reader.poll()[0].status(), Status::Completed);
}

#[test]
fn changed_session_identity_never_leaks_another_sessions_activity() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("root.jsonl");
    fs::write(&path, metadata("root", None)).unwrap();
    let mut reader = monitor(&dir);
    reader.discover().unwrap();
    let _ = reader.poll();
    let activity = serde_json::json!({"type":"event_msg","payload":{"type":"agent_message","message":"OTHER SESSION TEXT"}});
    fs::write(&path, metadata("other", None) + &format!("{activity}\n")).unwrap();
    let rows = reader.poll();
    assert_eq!(rows[0].activity(), None);
    assert_eq!(
        rows[0].freshness(),
        herdr_codex_subagents_rs::agent::Freshness::Stale
    );
}

#[test]
fn replacement_activity_at_same_offset_replaces_old_text() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("root.jsonl");
    let content = |message: &str| {
        metadata("root", None)
            + &format!(
                "{}\n",
                serde_json::json!({
        "type":"event_msg","payload":{"type":"agent_message","message":message}})
            )
    };
    fs::write(&path, content("old")).unwrap();
    let mut reader = monitor(&dir);
    reader.discover().unwrap();
    assert_eq!(reader.poll()[0].activity(), Some("old"));
    let new_path = dir.path().join("replacement.tmp");
    fs::write(&new_path, content("new")).unwrap();
    fs::remove_file(&path).unwrap();
    fs::rename(new_path, &path).unwrap();
    assert_eq!(reader.poll()[0].activity(), Some("new"));
}

#[test]
fn late_wait_cannot_steal_a_newer_turn() {
    let dir = TempDir::new().unwrap();
    let old_wait = serde_json::json!({"type":"event_msg","payload":{"type":"item_completed","turn_id":"r1",
        "item":{"type":"CollabAgentToolCall","tool":"wait"}}});
    fs::write(
        dir.path().join("root.jsonl"),
        metadata("root", None)
            + &event("task_started", "r1")
            + &event("task_started", "r2")
            + &format!("{old_wait}\n")
            + &event("task_complete", "r1"),
    )
    .unwrap();
    let mut reader = monitor(&dir);
    reader.discover().unwrap();
    assert_eq!(reader.poll()[0].status(), Status::Working);
}

#[test]
fn delayed_wait_cannot_reopen_a_completed_turn() {
    let dir = TempDir::new().unwrap();
    let old_wait = serde_json::json!({"type":"event_msg","payload":{"type":"item_completed","turn_id":"r1",
        "item":{"type":"CollabAgentToolCall","tool":"wait"}}});
    fs::write(
        dir.path().join("root.jsonl"),
        metadata("root", None)
            + &event("task_started", "r1")
            + &event("task_complete", "r1")
            + &format!("{old_wait}\n"),
    )
    .unwrap();
    let mut reader = monitor(&dir);
    reader.discover().unwrap();
    assert_eq!(reader.poll()[0].status(), Status::Completed);
}

#[test]
fn discovery_advances_past_first_ten_thousand_files() {
    let dir = TempDir::new().unwrap();
    for index in 0..10_005 {
        fs::write(
            dir.path().join(format!("{index:05}.jsonl")),
            metadata("unrelated", None),
        )
        .unwrap();
    }
    fs::write(dir.path().join("zzzz-root.jsonl"), metadata("root", None)).unwrap();
    let mut reader = monitor(&dir);
    reader.discover().unwrap();
    assert_eq!(reader.poll()[0].thread().as_str(), "root");
}
