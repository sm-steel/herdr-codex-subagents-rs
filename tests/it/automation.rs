use std::fs;
use std::path::Path;
use std::process::Command;

use assert_cmd::cargo::cargo_bin;
use tempfile::TempDir;

#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "integration harness runs real executables"
)]
fn sync_attaches_exact_resumed_session_once_and_checks_panel_identity() {
    let dir = TempDir::new().unwrap();
    let fake = compile_fake(dir.path()).unwrap();
    let sessions = dir.path().join("codex/sessions");
    fs::create_dir_all(&sessions).unwrap();
    fs::write(sessions.join("root.jsonl"), "{\"type\":\"session_meta\",\"payload\":{\"id\":\"11111111-1111-4111-8111-111111111111\"}}\n").unwrap();
    fs::write(dir.path().join("panel-terminal"), "monitor-1").unwrap();
    let run = || {
        Command::new(cargo_bin!("herdr-codex-subagents-rs"))
            .arg("sync")
            .env("HERDR_BIN_PATH", &fake)
            .env("HERDR_PLUGIN_STATE_DIR", dir.path().join("state"))
            .env("CODEX_HOME", dir.path().join("codex"))
            .env("HERDR_SOCKET_PATH", dir.path())
            .output()
            .unwrap()
    };
    let first = run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(run().status.success());
    assert_eq!(
        fs::read_to_string(dir.path().join("opens"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    fs::write(dir.path().join("transient-get-error"), "").unwrap();
    assert!(!run().status.success());
    assert_eq!(
        fs::read_to_string(dir.path().join("opens"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    fs::remove_file(dir.path().join("transient-get-error")).unwrap();
    fs::write(dir.path().join("panel-terminal"), "unrelated-reused-pane").unwrap();
    assert!(run().status.success());
    assert_eq!(
        fs::read_to_string(dir.path().join("opens"))
            .unwrap()
            .lines()
            .count(),
        2
    );
    let calls = fs::read_to_string(dir.path().join("opens")).unwrap();
    assert!(calls.contains("HCS_ROOT_THREAD=11111111-1111-4111-8111-111111111111"));
    assert!(calls.contains("--no-focus"));
    fs::remove_file(dir.path().join("state/attachments.json")).unwrap();
    fs::write(dir.path().join("fresh"), "").unwrap();
    assert!(run().status.success());
    assert_eq!(
        fs::read_to_string(dir.path().join("opens"))
            .unwrap()
            .lines()
            .count(),
        2
    );
    fs::write(dir.path().join("undetected"), "").unwrap();
    assert_cmd::Command::new(cargo_bin!("herdr-codex-subagents-rs"))
        .args(["hook", "--herdr-bin"]).arg(&fake).arg("--state-dir").arg(dir.path().join("state"))
        .env("HERDR_ENV", "1").env("HERDR_PANE_ID", "w1:p1").env("HERDR_SOCKET_PATH", dir.path())
        .env("CODEX_HOME", dir.path().join("codex")).env_remove("CODEX_THREAD_ID")
        .write_stdin(r#"{"hook_event_name":"SessionStart","session_id":"11111111-1111-4111-8111-111111111111","source":"startup"}"#)
        .assert().success().stdout("");
    assert_eq!(
        fs::read_to_string(dir.path().join("opens"))
            .unwrap()
            .lines()
            .count(),
        3
    );
    fs::write(dir.path().join("source-pane"), "w2:p4").unwrap();
    assert!(run().status.success());
    assert_eq!(
        fs::read_to_string(dir.path().join("opens"))
            .unwrap()
            .lines()
            .count(),
        3
    );
    fs::remove_file(dir.path().join("state/attachments.json")).unwrap();
    fs::write(dir.path().join("bad-open-reply"), "").unwrap();
    assert!(!run().status.success());
    assert!(run().status.success());
    assert_eq!(
        fs::read_to_string(dir.path().join("opens"))
            .unwrap()
            .lines()
            .count(),
        4
    );
}

#[test]
fn setup_preserves_other_hooks_and_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("hooks.json");
    fs::write(
        &path,
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"my-existing-hook"}]}]}}"#,
    )
    .unwrap();
    let executable = dir.path().join(if cfg!(windows) {
        "herdr-codex-subagents-rs.exe"
    } else {
        "herdr-codex-subagents-rs"
    });
    for _ in 0..2 {
        herdr_codex_subagents_rs::setup::install(
            dir.path(),
            &executable,
            &dir.path().join("herdr"),
            &dir.path().join("state"),
        )
        .unwrap();
    }
    let config: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        config["hooks"]["Stop"][0]["hooks"][0]["command"],
        "my-existing-hook"
    );
    assert_eq!(config["hooks"]["SessionStart"].as_array().unwrap().len(), 1);
    assert!(
        dir.path()
            .join("hooks.json.before-codex-subagents")
            .exists()
    );
    fs::write(&path, "malformed user config").unwrap();
    assert!(
        herdr_codex_subagents_rs::setup::install(
            dir.path(),
            &executable,
            &dir.path().join("herdr"),
            &dir.path().join("state")
        )
        .is_err()
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "malformed user config");
}

#[expect(
    clippy::disallowed_methods,
    reason = "integration harness compiles its executable fake"
)]
fn compile_fake(dir: &Path) -> std::io::Result<std::path::PathBuf> {
    let fake = dir.join(if cfg!(windows) { "fake.exe" } else { "fake" });
    assert!(
        Command::new("rustc")
            .args(["--edition", "2024"])
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/automation_fake.rs"))
            .arg("-o")
            .arg(&fake)
            .status()?
            .success()
    );

    Ok(fake)
}
