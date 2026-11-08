//! Command-line entry point for the read-only Herdr Codex monitor.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{self, Read, Write};
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
    /// Automatically attach monitors to identified Codex panes.
    Sync,
    /// Install the exact-identity hook; Codex requires its normal trust review.
    Setup,
    /// Receive an exact session-start identity from Codex (silent hook output).
    Hook {
        #[arg(long)]
        herdr_bin: PathBuf,
        #[arg(long)]
        state_dir: PathBuf,
    },
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
    let mut environment: BTreeMap<OsString, OsString> = std::env::vars_os().collect();
    let args = Args::parse();
    match args.command {
        Action::Hook {
            herdr_bin,
            state_dir,
        } => {
            if env(&environment, "HERDR_ENV").is_none_or(|value| value != "1") {
                return Ok(());
            }
            environment.insert("HERDR_BIN_PATH".into(), herdr_bin.into_os_string());
            environment.insert("HERDR_PLUGIN_STATE_DIR".into(), state_dir.into_os_string());
            hook(&environment)
        }
        Action::Sync => output(&sync(&environment)?),
        Action::Setup => {
            let host = env(&environment, "HERDR_BIN_PATH")
                .map(PathBuf::from)
                .ok_or_else(|| io::Error::other("Herdr executable context missing"))?;
            let state = env(&environment, "HERDR_PLUGIN_STATE_DIR")
                .map(PathBuf::from)
                .ok_or_else(|| io::Error::other("Herdr plugin state directory missing"))?;
            let sessions = sessions_dir(&environment)?;
            let home = sessions
                .parent()
                .ok_or_else(|| io::Error::other("Codex home unavailable"))?;
            herdr_codex_subagents_rs::setup::install(
                home,
                &std::env::current_exe()?,
                &host,
                &state,
            )?;
            output(&serde_json::json!({"configured":true,"codex_hook_review":"/hooks"}))
        }
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

fn sync(environment: &BTreeMap<OsString, OsString>) -> io::Result<serde_json::Value> {
    let binary = env(environment, "HERDR_BIN_PATH")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("Herdr executable context missing"))?;
    let state = env(environment, "HERDR_PLUGIN_STATE_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("Herdr plugin state directory missing"))?;
    let connection = format!(
        "{}|{}",
        env(environment, "HERDR_SOCKET_PATH")
            .map_or_else(String::new, |v| v.to_string_lossy().into_owned()),
        env(environment, "HERDR_SESSION")
            .map_or_else(String::new, |v| v.to_string_lossy().into_owned())
    );
    let client = HerdrClient::new(binary, child_env(environment));
    let sessions = sessions_dir(environment)?;
    for _ in 0..64 {
        let result = herdr_codex_subagents_rs::attachment::sync(
            &client,
            &state,
            &connection,
            &sessions,
            env(environment, "CODEX_HOME").map(OsString::as_os_str),
        )?;
        if result.get("more").and_then(serde_json::Value::as_bool) != Some(true) {
            return Ok(result);
        }
    }
    Err(io::Error::other("monitor attachment batch limit reached"))
}

fn hook(environment: &BTreeMap<OsString, OsString>) -> io::Result<()> {
    let mut bytes = Vec::new();
    io::stdin().take(65_537).read_to_end(&mut bytes)?;
    if bytes.len() > 65_536 {
        return Err(io::Error::other("hook input exceeds limit"));
    }
    let payload: serde_json::Value = serde_json::from_slice(&bytes)?;
    if payload
        .get("hook_event_name")
        .and_then(serde_json::Value::as_str)
        != Some("SessionStart")
    {
        return Ok(());
    }
    let Some(id) = payload
        .get("session_id")
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())
    else {
        return Ok(());
    };
    if env(environment, "CODEX_THREAD_ID").is_some_and(|thread| thread != id) {
        return Ok(());
    }
    let Some(pane) = env(environment, "HERDR_PANE_ID").and_then(|p| p.to_str()) else {
        return Ok(());
    };
    let binary = env(environment, "HERDR_BIN_PATH")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("Herdr executable context missing"))?;
    let client = HerdrClient::new(binary, child_env(environment));
    // The hook's session identity is exact; no folder or focus fallback is used.
    let mut detected = false;
    for _ in 0..20 {
        if client
            .query(&["agent".into(), "get".into(), pane.into()])
            .is_ok_and(|agent| {
                agent
                    .pointer("/result/agent/agent")
                    .and_then(serde_json::Value::as_str)
                    == Some("codex")
            })
        {
            detected = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    if !detected {
        return Err(io::Error::other(
            "Codex detection did not become ready for session hook",
        ));
    }
    client.report_session(
        pane,
        id,
        payload.get("source").and_then(serde_json::Value::as_str),
    )?;
    for _ in 0..10 {
        let result = sync(environment)?;
        if result.get("skipped").and_then(serde_json::Value::as_u64) == Some(0) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Ok(())
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
        "HERDR_PANE_ID",
        "HERDR_WORKSPACE_ID",
        "HERDR_TAB_ID",
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
