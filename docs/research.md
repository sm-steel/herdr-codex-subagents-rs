# Research and compatibility notes

Checked 2026-11-08. Primary sources are authoritative for contracts; local 0.9.5
CLI help wins over newer website examples. Recommendations remain design proposals.

## Herdr

- [Plugin documentation](https://herdr.dev/tui/docs/plugins/): executable packages
  with a manifest; Rust is supported. Use `HERDR_BIN_PATH` to delegate socket/pipe
  differences. Runtime state belongs outside managed plugin source directories.
- [Socket API](https://herdr.dev/tui/docs/socket-api/): raw transport is OS-specific;
  CLI wrappers are the recommended starting point. Agent view projections filter
  existing agent facts, not proof that arbitrary Codex child threads can be added.
- Installed `herdr plugin pane open --help` supports split placement, explicit
  target pane, direction, environment and no-focus. Installed `pane send-keys`
  delivers Alt+Left/Right, up/down and Enter. It rejected `end` in the spike;
  never assume the full current website key vocabulary works on 0.9.5.
- [Integrations](https://herdr.dev/tui/docs/integrations/): Codex integration reports
  session identity; screen detection supplies outer agent status. It does not
  establish per-child status as a Herdr API capability.

## Codex

- [Official app-server documentation](https://learn.chatgpt.com/docs/app-server):
  structured thread history/status and item notifications, including delegation
  items. Parent/ancestor filters can be experimental. A separate server reading
  files is not an observer of another server's live in-memory state.
- [Official commands](https://learn.chatgpt.com/docs/developer-commands?surface=cli):
  `/subagents` switches the active thread through a picker.
- [Versioned navigation helpers](https://github.com/openai/codex/blob/rust-v0.166.0/codex-rs/tui/src/multi_agents.rs):
  next/previous shortcuts are Alt+Right and Alt+Left.
- [Versioned input routing](https://github.com/openai/codex/blob/rust-v0.166.0/codex-rs/tui/src/app/input.rs):
  shortcuts require an empty composer and no modal/popup. This is a hard
  compatibility constraint, not something a plugin should bypass by deleting text.
- [Versioned picker](https://github.com/openai/codex/blob/rust-v0.166.0/codex-rs/tui/src/app/agent_picker.rs):
  refresh and selection are TUI-owned. No external direct select-thread control
  was established in this research.

## Rust patterns

- [Ratatui](https://docs.rs/ratatui/latest/ratatui/): standard terminal widgets,
  Crossterm input and terminal-lifetime helpers. Use a pure state update separated
  from drawing and external I/O. Mouse hit testing maps visible row rectangles
  to stable thread IDs, never names or stale indices.
- [notify](https://docs.rs/notify/latest/notify/): cross-platform notifications have
  filesystem-specific gaps, notably mounted/WSL paths. Proposed first version
  uses incremental polling; native notifications may later accelerate invalidation,
  with periodic reconciliation retained for correctness.
- [Rust testing](https://doc.rust-lang.org/book/ch11-03-test-organization.html):
  unit tests beside implementation; external integration tests exercise public API.

## Open questions requiring characterization

Exact v1/v2 JSONL lifecycle variants; reliable root identity from Herdr 0.9.5;
UUID verification from narrow/large picker layouts; arbitrary click navigation
from read-only child views; manual navigation and input races; completed children
after restart; Windows/Linux manifest executable resolution; active wait states.
These are explicit task gates, not assertions that portability is already proven.
