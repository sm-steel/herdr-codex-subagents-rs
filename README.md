# Herdr Codex Subagents (Rust)

A planned Windows and Linux Herdr plugin: one compact panel containing the main
Codex thread followed by its subagents. Click a row to inspect that thread in the
existing Codex pane. Each row shows status, assignment, and latest observable activity.

**Status: design and navigation proof of concept; no installable plugin yet.**

- [Design](docs/design.md): behavior, architecture, constraints, acceptance criteria.
- [Implementation plan](docs/plan.md): sequenced tasks and verification.
- [Research](docs/research.md): primary sources and compatibility findings.
- [PoC results](poc/RESULTS.md): tested capabilities and limitations.
- [Rust rules](docs/rust-rules.md): conventions adapted from semoxide.
- [Rule migration](docs/rule-migration.md): retained, adapted, and omitted policies.

The monitor will not call model APIs, require an API key, submit prompts, start
additional Codex agents, or stop existing agents. Thread selection is an explicit
terminal-navigation action. Agent text is sensitive local data and is not telemetry.

## Navigation spike

The dependency-free spike invokes Herdr's portable CLI and waits for a redraw.
It is a throwaway transport check, not the plugin or a reliable thread selector.

```powershell
rustc --edition 2024 poc/navigation.rs -o poc/navigation.exe
./poc/navigation.exe <herdr-executable> <isolated-session> <pane-id> next
./poc/navigation.exe <herdr-executable> <isolated-session> <pane-id> previous
```

Run only against a dedicated test pane. Shortcuts can be consumed by drafts or
popups. The production implementation must verify identity and preconditions.

## Development

Implementation begins after design review. The intended stack is Rust 2024,
Ratatui/Crossterm, serde, and Herdr's CLI. Rust binaries and real Windows/Linux CI
are required; Bash, PowerShell scripts, Python, and native Unix sockets must not
be runtime prerequisites. The initial repository contains no Cargo product skeleton.

Independent project; not affiliated with OpenAI or Herdr.
