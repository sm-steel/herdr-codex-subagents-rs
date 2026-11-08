# Compatibility evidence

## Current scope

The first version is monitoring only. Thread switching is disabled until an atomic
TUI navigation or input-exclusion mechanism is established. The old navigation
spike remains exploratory evidence, not product behavior.

| Environment | Evidence |
| --- | --- |
| Windows, Rust 1.99.0 | Unit/integration tests and strict Clippy run locally. |
| Windows, Herdr 0.9.5 | Manifest linked and actual release panel opened in an isolated named session. Extensionless executable path works. |
| Codex 0.166.0 | Synthetic real main/child rollouts observed, both completed states rendered. |
| Linux x64 | GitHub Actions checks are configured; see runs for actual results. Local WSL cannot start because virtualization is disabled. |
| Rust MSRV 1.97 | CI job configured; verify its result before claiming support. |
| Linux ARM64/macOS | Not claimed. |

## Observation mapping

- `session_meta` supplies exact thread and parent identities, including nested
  legacy thread-spawn metadata. Names/agent paths are presentation, never identity.
- `task_started`, `task_complete`, `turn_aborted`/`task_interrupted` and
  `task_failed` represent explicit lifecycle states.
- v2 item messages provide user task and latest assistant activity. Waiting
  delegation items refine an active turn into waiting until the wait completes.
- v2 `spawn_agent` function calls correlate with `SubAgentActivity` by call ID,
  so concurrent agents do not receive each other's assignments.
- Missing upstream events cannot establish waiting, failure, closed, or idle.
  No filename age or silence heuristic manufactures those states.

Current gaps: legacy-v1 delegation assignment coverage, full approval-wait coverage,
cross-host observation, and process-liveness verification. Do not confuse the
recorded lifecycle with Herdr's outer-pane process classification.
