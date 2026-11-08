//! Installation of the one exact-identity Codex hook needed by automatic attachment.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use serde_json::{Value, json};

/// Install or refresh this plugin's hook while preserving all other hook entries.
///
/// Codex still requires its normal human trust review before the hook can run.
///
/// # Errors
/// Returns an error for unsupported hook config or an unwritable config directory.
pub fn install(home: &Path, executable: &Path, host: &Path, state: &Path) -> io::Result<()> {
    fs::create_dir_all(home)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(home.join(".codex-subagents-hooks.lock"))?;
    lock.lock()?;
    let path = home.join("hooks.json");
    let original = fs::read(&path).ok();
    let old = match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice::<Value>(&bytes)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => json!({}),
        Err(error) => return Err(error),
    };
    let mut config = old.clone();
    let root = config
        .as_object_mut()
        .ok_or_else(|| io::Error::other("Codex hooks config must be an object"))?;
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| io::Error::other("Codex hooks must be an object"))?;
    let starts = hooks
        .entry("SessionStart")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| io::Error::other("Codex SessionStart hooks must be an array"))?;
    for group in &mut *starts {
        if let Some(handlers) = group.get_mut("hooks").and_then(Value::as_array_mut) {
            handlers.retain(|handler| {
                !handler
                    .get("command")
                    .and_then(Value::as_str)
                    .is_some_and(|command| {
                        command.contains("herdr-codex-subagents-rs")
                            && command.contains(" hook --herdr-bin ")
                    })
            });
        }
    }
    starts.retain(|group| {
        group
            .get("hooks")
            .and_then(Value::as_array)
            .is_none_or(|handlers| !handlers.is_empty())
    });
    let command = format!(
        "{} hook --herdr-bin {} --state-dir {}",
        quote(executable)?,
        quote(host)?,
        quote(state)?
    );
    starts.push(json!({"hooks":[{"type":"command","command":command,"timeout":30,"statusMessage":"Attaching Codex agents panel"}]}));
    if config == old {
        return Ok(());
    }
    fs::create_dir_all(home)?;
    if path.exists() && !home.join("hooks.json.before-codex-subagents").exists() {
        fs::copy(&path, home.join("hooks.json.before-codex-subagents"))?;
    }
    let mut file = tempfile::NamedTempFile::new_in(home)?;
    serde_json::to_writer_pretty(file.as_file_mut(), &config)?;
    writeln!(file)?;
    if fs::read(&path).ok() != original {
        return Err(io::Error::other(
            "Codex hooks changed during setup; retry configuration",
        ));
    }
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

fn quote(path: &Path) -> io::Result<String> {
    let text = path
        .to_str()
        .ok_or_else(|| io::Error::other("hook executable paths must be UTF-8"))?;
    if cfg!(windows) {
        if text.contains(['"', '%', '\r', '\n']) {
            return Err(io::Error::other(
                "hook path contains unsupported shell characters",
            ));
        }
        Ok(format!("\"{text}\""))
    } else {
        Ok(format!("'{}'", text.replace('\'', "'\"'\"'")))
    }
}
