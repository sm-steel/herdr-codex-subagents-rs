use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use herdr_codex_subagents_rs::herdr::HerdrClient;
use tempfile::TempDir;

#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "the integration harness compiles a real fake executable"
)]
fn inherited_stdout_does_not_hold_the_query_open() {
    let dir = TempDir::new().unwrap();
    let executable = dir.path().join(if cfg!(windows) {
        "herdr-fake.exe"
    } else {
        "herdr-fake"
    });
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/herdr_fake.rs");
    assert!(
        Command::new("rustc")
            .args(["--edition", "2024"])
            .arg(source)
            .arg("-o")
            .arg(&executable)
            .status()
            .unwrap()
            .success()
    );
    let marker = dir.path().join("hold");
    std::fs::write(&marker, "hold").unwrap();
    let client = HerdrClient::new(executable, BTreeMap::new());
    let started = Instant::now();
    let result = client.query(&[marker.clone().into_os_string()]).unwrap();
    let duration = started.elapsed();
    std::fs::remove_file(&marker).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !marker.with_extension("done").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(result["result"]["ok"], true);
    assert!(
        duration < Duration::from_secs(2),
        "stdout collection exceeded the query's direct-exit budget"
    );
    assert!(
        marker.with_extension("done").exists(),
        "test descendant did not exit"
    );
}
