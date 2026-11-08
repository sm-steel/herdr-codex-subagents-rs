//! Bounded Herdr CLI queries, using the host's own socket transport.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{self, Read};
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
    #[expect(
        clippy::disallowed_methods,
        reason = "the Herdr adapter exclusively owns subprocess creation"
    )]
    pub fn query(&self, args: &[OsString]) -> io::Result<Value> {
        let mut child = Command::new(&self.binary)
            .args(args)
            .env_clear()
            .envs(self.environment.iter())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let output = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("missing Herdr stdout"))?;
        thread::scope(|scope| {
            let reader = scope.spawn(move || {
                let mut bytes = Vec::new();
                output.take(65_537).read_to_end(&mut bytes)?;
                Ok::<_, io::Error>(bytes)
            });
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
            }
            let bytes = reader
                .join()
                .map_err(|_| io::Error::other("Herdr output reader failed"))??;
            result?;
            if bytes.len() > 65_536 {
                return Err(io::Error::other("Herdr output exceeded limit"));
            }
            serde_json::from_slice(&bytes)
                .map_err(|_| io::Error::other("Herdr returned invalid JSON"))
        })
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
        Ok(agent
            .pointer("/agent_session/value")
            .and_then(Value::as_str)
            .map(str::to_owned))
    }
}
