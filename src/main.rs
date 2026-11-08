//! Command-line entry point for the read-only Herdr Codex monitor.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use herdr_codex_subagents_rs::agent::ThreadId;
use herdr_codex_subagents_rs::herdr::HerdrClient;
use herdr_codex_subagents_rs::rollout::Monitor;
use herdr_codex_subagents_rs::{panel, text::display_text};

#[derive(Parser)]
#[command(version, about = "Read-only Codex main/subagent monitor for Herdr")]
struct Args {
    #[command(subcommand)]
    command: Action,
}

#[derive(Subcommand)]
enum Action {
    /// Show local observation prerequisites, without reading credentials.
    Doctor,
    /// Open a monitor pane next to the explicitly identified Codex pane.
    Open {
        #[arg(long)]
        root_thread: Option<String>,
        #[arg(long)]
        target_pane: Option<String>,
    },
    /// Observe an exact root thread; JSON mode works without a terminal.
    Watch {
        #[arg(long)]
        root_thread: Option<String>,
        #[arg(long)]
        sessions_dir: Option<PathBuf>,
        #[arg(long)]
        once: bool,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(error) => {
            report_error(&error);
            ExitCode::FAILURE
        }
    }
}

#[expect(
    clippy::disallowed_methods,
    reason = "the CLI snapshots ambient environment once"
)]
fn run() -> io::Result<()> {
    let environment: BTreeMap<OsString, OsString> = std::env::vars_os().collect();
    let args = Args::parse();
    match args.command {
        Action::Doctor => output(
            &serde_json::json!({"sessions_directory_available": sessions_dir(&environment)?.is_dir(),
                "herdr_context": env(&environment, "HERDR_BIN_PATH").is_some(),
                "mode":"read-only", "thread_switching":false}),
        ),
        Action::Watch {
            root_thread,
            sessions_dir: directory,
            once,
        } => {
            let root = root_thread
                .or_else(|| {
                    env(&environment, "HCS_ROOT_THREAD").map(|v| v.to_string_lossy().into_owned())
                })
                .ok_or_else(|| {
                    io::Error::other("provide --root-thread; no session is chosen by directory")
                })?;
            let mut monitor = Monitor::new(
                directory.map_or_else(|| sessions_dir(&environment), Ok)?,
                ThreadId::new(root),
            );
            monitor.discover()?;
            if once {
                let rows: Vec<_> = monitor
                    .poll()
                    .iter()
                    .map(|row| {
                        serde_json::json!({
                    "thread_id":row.thread().as_str(), "depth":row.depth(), "name":row.name(),
                    "status":row.status().marker(), "freshness":format!("{:?}",row.freshness()),
                    "task":row.assignment(), "activity":row.activity()})
                    })
                    .collect();
                output(
                    &serde_json::json!({"agents":rows,"diagnostics":monitor.warnings(),"thread_switching":false}),
                )
            } else {
                panel::run(monitor)
            }
        }
        Action::Open {
            root_thread,
            target_pane,
        } => open(&environment, root_thread, target_pane),
    }
}

fn open(
    environment: &BTreeMap<OsString, OsString>,
    root: Option<String>,
    pane: Option<String>,
) -> io::Result<()> {
    let binary = env(environment, "HERDR_BIN_PATH")
        .map(PathBuf::from)
        .ok_or_else(|| {
            io::Error::other("open must run as a Herdr plugin action (HERDR_BIN_PATH missing)")
        })?;
    let pane = pane
        .or_else(|| {
            env(environment, "HERDR_PANE_ID").map(|value| value.to_string_lossy().into_owned())
        })
        .ok_or_else(|| io::Error::other("provide --target-pane"))?;
    let client = HerdrClient::new(binary, child_env(environment));
    let recorded = client.codex_session(&pane)?;
    let root = root.or(recorded).ok_or_else(|| io::Error::other(
        "Codex session identity unavailable; install Herdr's Codex integration or provide --root-thread"))?;
    let mut monitor = Monitor::new(sessions_dir(environment)?, ThreadId::new(root.clone()));
    monitor.discover()?;
    let args = [
        "plugin",
        "pane",
        "open",
        "--plugin",
        "sm-steel.codex-subagents",
        "--entrypoint",
        "monitor",
        "--placement",
        "split",
        "--target-pane",
        &pane,
        "--direction",
        "right",
        "--no-focus",
        "--env",
    ];
    let mut args: Vec<OsString> = args.into_iter().map(OsString::from).collect();
    args.push(format!("HCS_ROOT_THREAD={root}").into());
    if let Some(home) = env(environment, "CODEX_HOME") {
        args.push("--env".into());
        let mut value = OsString::from("CODEX_HOME=");
        value.push(home);
        args.push(value);
    }
    output(&client.query(&args)?)
}

fn env<'a>(environment: &'a BTreeMap<OsString, OsString>, name: &str) -> Option<&'a OsString> {
    environment
        .iter()
        .find(|(key, _)| {
            if cfg!(windows) {
                key.to_string_lossy().eq_ignore_ascii_case(name)
            } else {
                key.as_os_str() == name
            }
        })
        .map(|(_, value)| value)
}

fn sessions_dir(environment: &BTreeMap<OsString, OsString>) -> io::Result<PathBuf> {
    if let Some(home) = env(environment, "CODEX_HOME") {
        return Ok(PathBuf::from(home).join("sessions"));
    }
    let home = env(
        environment,
        if cfg!(windows) { "USERPROFILE" } else { "HOME" },
    )
    .ok_or_else(|| io::Error::other("home directory unavailable; provide --sessions-dir"))?;
    Ok(PathBuf::from(home).join(".codex/sessions"))
}

fn child_env(environment: &BTreeMap<OsString, OsString>) -> BTreeMap<OsString, OsString> {
    let allowed = [
        "PATH",
        "PATHEXT",
        "SYSTEMROOT",
        "WINDIR",
        "APPDATA",
        "LOCALAPPDATA",
        "USERPROFILE",
        "HOME",
        "TEMP",
        "TMP",
        "HERDR_SOCKET_PATH",
        "HERDR_SESSION",
        "HERDR_ENV",
        "CODEX_HOME",
    ];
    environment
        .iter()
        .filter(|(key, _)| {
            allowed
                .iter()
                .any(|name| key.to_string_lossy().eq_ignore_ascii_case(name))
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

#[expect(
    clippy::disallowed_methods,
    reason = "CLI output owns stdout; the library never prints"
)]
fn output(value: &serde_json::Value) -> io::Result<()> {
    let mut output = io::stdout().lock();
    serde_json::to_writer(&mut output, value)?;
    writeln!(output)
}

#[expect(clippy::disallowed_methods, reason = "CLI diagnostics own stderr")]
fn report_error(error: &io::Error) {
    let _ = writeln!(
        io::stderr().lock(),
        "codex-subagents: {}",
        display_text(&error.to_string(), 512)
    );
}
