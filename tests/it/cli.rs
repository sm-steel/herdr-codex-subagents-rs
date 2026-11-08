use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn json_watch_reports_exact_root_and_read_only_mode() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("root.jsonl"),
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"root\"}}\n",
    )
    .unwrap();
    let output = cargo_bin_cmd!("herdr-codex-subagents-rs")
        .args(["watch", "--root-thread", "root", "--sessions-dir"])
        .arg(dir.path())
        .arg("--once")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["agents"][0]["thread_id"], "root");
    assert_eq!(value["thread_switching"], false);
}
#[test]
fn missing_root_is_an_error_not_an_implicit_newest_session() {
    cargo_bin_cmd!("herdr-codex-subagents-rs")
        .env_remove("HCS_ROOT_THREAD")
        .args(["watch", "--once"])
        .assert()
        .failure();
}
#[test]
fn unknown_root_fails_without_printing_conversation_content() {
    let dir = TempDir::new().unwrap();
    let output = cargo_bin_cmd!("herdr-codex-subagents-rs")
        .args(["watch", "--root-thread", "missing", "--sessions-dir"])
        .arg(dir.path())
        .arg("--once")
        .assert()
        .failure()
        .get_output()
        .clone();
    assert_eq!(output.stdout, Vec::<u8>::new());
}
#[test]
fn open_without_host_context_fails_without_opening_a_pane() {
    cargo_bin_cmd!("herdr-codex-subagents-rs")
        .env_remove("HERDR_BIN_PATH")
        .arg("open")
        .assert()
        .failure();
}

#[test]
fn explicit_sessions_directory_does_not_require_a_home_environment() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("root.jsonl"),
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"root\"}}\n",
    )
    .unwrap();
    cargo_bin_cmd!("herdr-codex-subagents-rs")
        .env_remove("HOME")
        .env_remove("USERPROFILE")
        .env_remove("CODEX_HOME")
        .args(["watch", "--root-thread", "root", "--sessions-dir"])
        .arg(dir.path())
        .arg("--once")
        .assert()
        .success();
}
