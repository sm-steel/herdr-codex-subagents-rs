//! Throwaway bridge: deliver native navigation keys through an explicit Herdr session.
#![forbid(unsafe_code)]

use std::error::Error;
use std::io::{self, Write};
use std::process::{Command, ExitCode};
use std::time::Duration;

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let binary = args.next().ok_or("missing Herdr executable")?;
    let session = args.next().ok_or("missing isolated session")?;
    let pane = args.next().ok_or("missing test pane")?;
    let action = args.next().ok_or("missing action: next or previous")?;
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }
    let key = match action.to_str() {
        Some("next") => "alt+right",
        Some("previous") => "alt+left",
        _ => return Err("action must be next or previous".into()),
    };
    let status = Command::new(&binary)
        .arg("--session")
        .arg(&session)
        .args(["pane", "send-keys"])
        .arg(&pane)
        .arg(key)
        .status()?;
    if !status.success() {
        return Err("Herdr rejected key delivery".into());
    }
    std::thread::sleep(Duration::from_millis(1000));
    let output = Command::new(&binary)
        .arg("--session")
        .arg(&session)
        .args(["pane", "read"])
        .arg(&pane)
        .args(["--source", "visible", "--lines", "16"])
        .output()?;
    if !output.status.success() {
        return Err("Herdr could not read test pane".into());
    }
    io::stdout().write_all(&output.stdout)?;
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr(), "navigation spike: {error}");
            ExitCode::FAILURE
        }
    }
}
