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
updating. On Windows, close running monitor panes before rebuilding the executable.
GitHub installs run the manifest's Cargo build command.

## Automatic panels

The plugin opens one 36-column monitor beside each exactly identified Codex pane,
without moving focus. Main and descendants share a connected tree, task/activity
previews and animated working icons. Repeated startup/events reuse existing panels.

After linking into a running server, run the **Configure automatic Codex panels**
action once. It preserves other Codex hooks and installs a silent Rust `SessionStart`
hook. Review and trust that hook in Codex `/hooks`; Codex requires this human step.
The action is available as `herdr plugin action invoke sm-steel.codex-subagents.setup`.
Then **Attach Codex monitors automatically** attaches existing resumed chats. Both
configuration and attachment also run automatically at future Herdr server starts.

No thread UUIDs need to be entered. Exact identity comes from the trusted hook or
an unambiguous `codex resume <UUID>` process argument. An already-running fresh
session without either identity remains untouched until the hook reports it.
Never select sessions by directory or recency. One panel lists all descendants;
completed agents stay visible. New monitors default to 36 terminal columns where
Herdr's minimum pane sizes permit; later window resizing follows Herdr split ratios.

The hook uses Herdr's native session-reporting protocol, then reconciles panels.
Startup/detection/status/move events provide additional reconciliation. If pane
creation has an ambiguous failure, the registry marks it pending and suppresses
retries rather than opening duplicates; diagnostics report the skipped attachment.
Moving a source retains its existing monitor, which may remain in its original tab.

Developer commands `watch --root-thread <UUID> [--once]` and `open` remain available
for explicit observation and diagnostics. `--once` prints JSON; `--sessions-dir`
selects a fixture/custom log directory. Defaults are `CODEX_HOME/sessions` or
`~/.codex/sessions`. `doctor` checks prerequisites without reading credentials.

## Tree and controls

Main stays pinned first with a right-aligned `[working of total]` descendant count.
Active branches sort first. The animated four-row Braille icon also marks parents
with active descendants. Other rows show a gray `⣿`, with one space on each side.
Tree connectors, selection, text and footer use Catppuccin Mocha colors.

Up/down selects, left/right collapses/expands or follows parent/child, and
Enter/Space or a mouse click toggles a branch. `q` or Ctrl-C exits only the monitor.
The two-row footer has a separate background; no extra header or outer border.

Status is the last recorded lifecycle, not proof that a process is still running.
Quiet logs never imply completion. Source failures show stale. Task and activity
remain separate. JSON observation retains every distinct lifecycle outcome.

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

Main starts expanded; nested subagent branches start collapsed. Expanding a branch
keeps it expanded across updates. Hidden descendants still affect icons and counts.
