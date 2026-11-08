# Herdr Codex Subagents (Rust)

One read-only Herdr panel for the main Codex thread and its subagents, with status
icons, recorded tasks and latest observable activity. Windows and Linux builds.

**Preview:** monitoring works; thread switching is deliberately disabled. Clicking
a row never sends keys or prompts to Codex. The original Codex TUI stays in its pane.

## Install from source

Requirements: Rust 1.99.0 (installed by rustup from `rust-toolchain.toml`), Cargo,
Herdr 0.9.5+, and local Codex rollout files. Initial format baseline: Codex 0.166.0.
No API key, subscription change, Python, or shell script is needed by the monitor.

```powershell
git clone https://github.com/sm-steel/herdr-codex-subagents-rs
cd herdr-codex-subagents-rs
cargo build --release --locked
herdr plugin link .
```

`plugin link` registers a local checkout; it does not build it. Rebuild after
updating. GitHub installs run the manifest's Cargo build command.

## Open the panel

Run the **Show Codex subagents** plugin action on your Codex pane. It uses Herdr's
recorded session identity when available. Herdr's Codex integration must already
be installed and must have reported the session; this plugin does not change your
Codex hooks or configuration.

For a session without that identity, run from inside the source Herdr pane:

```powershell
./target/release/herdr-codex-subagents-rs open --root-thread <Codex-thread-UUID> --target-pane <Herdr-pane-ID>
```

On Windows `.exe` is optional. The root is the main thread UUID, not a child UUID.
No session is selected by folder or by recency. A wrong or absent root fails safely.
Each open invocation currently creates a panel; close an old panel before reopening.

To observe without plugin context:

```powershell
./target/release/herdr-codex-subagents-rs watch --root-thread <UUID>
./target/release/herdr-codex-subagents-rs watch --root-thread <UUID> --once
```

`--once` prints JSON and exits. `--sessions-dir <path>` points to a test or custom
rollout directory. Otherwise `CODEX_HOME/sessions` or the platform home directory's
`.codex/sessions` is used. `doctor` prints prerequisite availability without reading
credentials. Linux uses the same commands.

## Status and keys

- Main is pinned first, with descendants indented below it.
- Working animates; waiting, idle, completed, interrupted, failed, closed and
  unknown have distinct text markers. Color is supplementary.
- Status is the last **recorded lifecycle**, not proof that a process is still
  running. Quiet logs never imply completion. Source read failures show stale.
- Task and latest activity are separate; unavailable data stays unavailable.
- Up/down scroll children; `q` or Ctrl-C exits only the monitor.

## Limits and privacy

The rollout format is internal and may change. Unknown records are ignored;
malformed/oversized records increment a diagnostic count. Polling is incremental
with bounded records and per-poll work. Long session archives may take time to index.
Only local root/descendant records are rendered. No telemetry or model API requests.

Conversation text may contain secrets. It is shown locally in the panel, stripped
of terminal control sequences. No conversation content is written to debug logs.
The monitor does not read auth files, control agents, approve requests, or modify
rollouts. SSH observation must run on the machine owning the files.

## Development

```powershell
mise install
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
mise exec -- cargo-deny check
```

Install local hooks with `mise exec -- lefthook install`. Pure-core tests require
maintainer review before implementation; see [Rust rules](docs/rust-rules.md).

- [Design](docs/design.md)
- [Implementation plan](docs/plan.md)
- [Research](docs/research.md)
- [Compatibility](docs/compatibility.md)
- [Rule migration from semoxide](docs/rule-migration.md)
- [Navigation PoC](poc/RESULTS.md)

MIT OR Apache-2.0. Independent project, not affiliated with OpenAI or Herdr.
