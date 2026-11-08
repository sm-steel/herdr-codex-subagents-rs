//! Bounded Herdr CLI queries, using the host's own socket transport.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{self, Read};
use std::io::{Seek, SeekFrom};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

/// Explicit executable and environment for read queries and plugin pane creation.
pub struct HerdrClient {
    binary: PathBuf,
    environment: BTreeMap<OsString, OsString>,
}

impl HerdrClient {
    /// Construct a client without reading ambient environment in the library.
    #[must_use]
    pub const fn new(binary: PathBuf, environment: BTreeMap<OsString, OsString>) -> Self {
        Self {
            binary,
            environment,
        }
    }

    /// Run an argv command with a deadline and capped output.
    ///
    /// # Errors
    /// Returns sanitized I/O diagnostics on timeout, rejection, or invalid JSON.
    pub fn query(&self, args: &[OsString]) -> io::Result<Value> {
        serde_json::from_slice(&self.execute(args)?)
            .map_err(|_| io::Error::other("Herdr returned invalid JSON"))
    }

    /// Report an exact Codex session through Herdr's built-in integration protocol.
    ///
    /// # Errors
    /// Returns an error if the host rejects the notification or times out.
    pub fn report_session(&self, pane: &str, root: &str, source: Option<&str>) -> io::Result<()> {
        let elapsed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(io::Error::other)?;
        let ticks = (elapsed.as_nanos() / 100 + 621_355_968_000_000_000).to_string();
        let mut args: Vec<OsString> = [
            "pane",
            "report-agent-session",
            pane,
            "--source",
            "herdr:codex",
            "--agent",
            "codex",
            "--seq",
            &ticks,
            "--agent-session-id",
            root,
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        if let Some(source) = source {
            args.extend(["--session-start-source".into(), source.into()]);
        }
        self.execute(&args)?;
        Ok(())
    }

    #[expect(
        clippy::disallowed_methods,
        reason = "the Herdr adapter exclusively owns subprocess creation"
    )]
    fn execute(&self, args: &[OsString]) -> io::Result<Vec<u8>> {
        // A private file cannot block at EOF when a descendant inherits stdout.
        let mut output = tempfile::tempfile()?;
        let mut errors = tempfile::tempfile()?;
        let mut child = Command::new(&self.binary)
            .args(args)
            .env_clear()
            .envs(self.environment.iter())
            .stdin(Stdio::null())
            .stdout(Stdio::from(output.try_clone()?))
            .stderr(Stdio::from(errors.try_clone()?))
            .spawn()?;
        let deadline = Instant::now() + Duration::from_secs(10);
        let result = loop {
            match child.try_wait() {
                Ok(Some(status)) if status.success() => break Ok(()),
                Ok(Some(_)) => {
                    break Err(io::Error::other(
                        "Herdr command rejected; check session and pane identity",
                    ));
                }
                Err(error) => break Err(error),
                Ok(None) if Instant::now() >= deadline => {
                    break Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "Herdr query timed out",
                    ));
                }
                Ok(None) => thread::sleep(Duration::from_millis(10)),
            }
        };
        if result.is_err() {
            let _ = child.kill();
            let _ = child.wait();
            errors.seek(SeekFrom::Start(0))?;
            let mut bytes = Vec::new();
            errors.take(65_537).read_to_end(&mut bytes)?;
            if bytes.len() <= 65_536
                && serde_json::from_slice::<Value>(&bytes)
                    .ok()
                    .and_then(|value| {
                        value
                            .pointer("/error/code")
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                    })
                    .as_deref()
                    == Some("pane_not_found")
            {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "Herdr pane no longer exists",
                ));
            }
        }
        result?;
        output.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        output.take(65_537).read_to_end(&mut bytes)?;
        if bytes.len() > 65_536 {
            return Err(io::Error::other("Herdr output exceeded limit"));
        }
        Ok(bytes)
    }

    /// Open a read-only panel beside an exact source pane, preserving focus.
    ///
    /// # Errors
    /// Returns a host query error if pane creation fails.
    pub fn open_monitor(
        &self,
        pane: &str,
        root: &str,
        codex_home: Option<&std::ffi::OsStr>,
    ) -> io::Result<Value> {
        let mut args: Vec<OsString> = [
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
            pane,
            "--direction",
            "right",
            "--no-focus",
            "--env",
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        args.push(format!("HCS_ROOT_THREAD={root}").into());
        if let Some(home) = codex_home {
            args.push("--env".into());
            let mut value = OsString::from("CODEX_HOME=");
            value.push(home);
            args.push(value);
        }
        let reply = self.query(&args)?;
        if let Some(panel) = reply
            .pointer("/result/plugin_pane/pane/pane_id")
            .and_then(Value::as_str)
        {
            let _ = self.size_monitor(panel);
        }
        Ok(reply)
    }

    /// Set a newly created right-hand monitor to 36 columns where the layout permits.
    ///
    /// # Errors
    /// Returns an error for unavailable geometry or a rejected resize.
    pub fn size_monitor(&self, pane: &str) -> io::Result<()> {
        for _ in 0..3 {
            let reply =
                self.query(&["pane".into(), "layout".into(), "--pane".into(), pane.into()])?;
            let layout = reply
                .pointer("/result/layout")
                .ok_or_else(|| io::Error::other("pane geometry unavailable"))?;
            let rect = layout
                .get("panes")
                .and_then(Value::as_array)
                .and_then(|panes| panes.iter().find(|p| p["pane_id"] == pane))
                .and_then(|pane| pane.get("rect"))
                .ok_or_else(|| io::Error::other("monitor geometry unavailable"))?;
            let width = coordinate(rect, "width")?;
            if width == 36 {
                break;
            }
            let x = coordinate(rect, "x")?;
            let split_width = layout
                .get("splits")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|split| split["direction"] == "right")
                .filter_map(|split| {
                    let rect = split.get("rect")?;
                    let start = coordinate(rect, "x").ok()?;
                    let span = coordinate(rect, "width").ok()?;
                    (start <= x
                        && u32::from(start) + u32::from(span) == u32::from(x) + u32::from(width))
                    .then_some(span)
                })
                .min()
                .filter(|span| *span > 0)
                .ok_or_else(|| io::Error::other("monitor split unavailable"))?;
            let amount = (f64::from(width.abs_diff(36)) / f64::from(split_width)).to_string();
            self.query(&[
                "pane".into(),
                "resize".into(),
                "--pane".into(),
                pane.into(),
                "--direction".into(),
                if width > 36 { "right" } else { "left" }.into(),
                "--amount".into(),
                amount.into(),
            ])?;
        }
        Ok(())
    }

    /// Inspect an exact Codex pane, never whichever pane happens to be focused.
    ///
    /// # Errors
    /// Returns an error if the pane is absent or no longer contains Codex.
    pub fn codex_session(&self, pane: &str) -> io::Result<Option<String>> {
        let result = self.query(&["agent".into(), "get".into(), pane.into()])?;
        let agent = result
            .pointer("/result/agent")
            .ok_or_else(|| io::Error::other("Herdr agent response is unsupported"))?;
        if agent.get("agent").and_then(Value::as_str) != Some("codex") {
            return Err(io::Error::other("target pane does not contain Codex"));
        }
        let recorded = agent
            .pointer("/agent_session/value")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if recorded.is_some() {
            return Ok(recorded);
        }
        let processes = self.query(&[
            "pane".into(),
            "process-info".into(),
            "--pane".into(),
            pane.into(),
        ])?;
        let roots: std::collections::BTreeSet<_> = processes
            .pointer("/result/process_info/foreground_processes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(resumed_thread)
            .collect();
        Ok((roots.len() == 1)
            .then(|| roots.into_iter().next())
            .flatten())
    }
}

fn valid_thread_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}

fn resumed_thread(process: &Value) -> Option<String> {
    let argv = process.get("argv")?.as_array()?;
    let executable = argv.first()?.as_str()?.rsplit(['/', '\\']).next()?;
    if !matches!(executable, "codex" | "codex.exe") || argv.get(1)?.as_str()? != "resume" {
        return None;
    }
    let id = argv.get(2)?.as_str()?;
    valid_thread_id(id).then(|| id.to_owned())
}

fn coordinate(rect: &Value, key: &str) -> io::Result<u16> {
    rect.get(key)
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| io::Error::other("invalid pane geometry"))
}
