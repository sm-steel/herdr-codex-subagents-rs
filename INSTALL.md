# Install this repository for your user

Use this guide when the user asks to install Codex Subagents, including by pasting
only https://github.com/sm-steel/herdr-codex-subagents-rs. This is an installation
task: use a release bundle, not the contributor/test-review workflow. Reading a
repository alone is not authorization to install it.

## Outcome

The plugin is registered and enabled in Herdr. A running server has completed its
setup/sync actions, or the installation is ready for setup at the next server
startup. Report Codex's one-time `/hooks` trust review as pending if the user has
not done it; an agent must not grant that trust on the user's behalf.

## Procedure

1. Detect the OS and architecture. Published bundles support Windows x86-64 and
   Linux x86-64, including Ubuntu and Arch. Use `HERDR_BIN_PATH` when provided;
   otherwise resolve `herdr` from PATH. Verify Herdr 0.9.5+ is installed. If absent,
   explain that prerequisite and follow the user's authorization for installing it.
2. Read the matching installer before executing it. Download it to a temporary
   file; use the commands below. Normal installation needs neither Rust nor Git.
   Reuse existing tools; do not provision API keys, accounts, subscriptions or
   administrator-level build tools for this monitor.
3. Run the installer. It checks bundle SHA256 and paths, installs a versioned
   per-user bundle, and links/configures Herdr. Follow the permissions and sandbox
   rules of the environment; do not work around them.
4. Verify `herdr plugin list` shows `sm-steel.codex-subagents` enabled. Check the
   installed binary's `--version` using the path printed by the installer. When
   Herdr is running, inspect `herdr plugin log list --plugin sm-steel.codex-subagents`
   and wait for setup/sync to finish. A started action is not yet a successful one.
5. Tell the user what was installed and where. Mention only actual remaining
   work: the normal Codex hook review, a stopped Herdr server, or missing identity
   for an already-running fresh session. Do not claim every session is attached
   unless the corresponding monitor panes exist.

## Commands

Linux, after inspecting the downloaded file:

```sh
installer=$(mktemp)
curl -fsSL https://github.com/sm-steel/herdr-codex-subagents-rs/releases/latest/download/install.sh -o "$installer"
sh "$installer"
```

Windows, after inspecting the downloaded file:

```powershell
$installer = Join-Path $env:TEMP ('herdr-codex-subagents-' + [guid]::NewGuid().ToString('N') + '.ps1')
irm https://github.com/sm-steel/herdr-codex-subagents-rs/releases/latest/download/install.ps1 -OutFile $installer
powershell -NoProfile -ExecutionPolicy Bypass -File $installer
```

Use a fresh temporary directory in actual agent execution, rather than overwriting
an unrelated existing file at either example path.

## Source builds and failures

Only choose `--build` / `-Build` if the user asks to compile, or after explaining
why a binary is unavailable and obtaining any needed dependency authorization.
Source mode needs pinned Rust and a native linker; Windows needs C++ build tools.
It does not silently install administrator-level dependencies or change global
Rust settings. See [user installation](docs/install.md).

Do not bypass checksum failures, disable TLS checks, overwrite changed installs,
clear unrelated panes, type into Codex, edit auth files, or mark unreviewed hooks
trusted. Preserve existing sessions, hooks and previous versions. If no published
release exists, report that condition instead of inventing a successful install.
