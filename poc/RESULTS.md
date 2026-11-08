# Navigation PoC

Date: 2026-11-08. Windows host, Herdr 0.9.5, official Codex CLI 0.166.0,
PowerShell 7. Isolated named Herdr server, explicitly created test pane and
project folder; existing user sessions were not controlled. One short model
request spawned one native child with a literal response; the monitor itself
made no model/API calls. Private rollouts and raw logs are excluded from Git.

## Observed

1. Official Codex ran inside a Herdr pane, created one native child and retained it.
2. `herdr ... pane send-keys <pane> alt+right` switched the existing TUI to the
   child's transcript. It displayed the child marker and disabled direct input.
3. A resumed parent initially did not switch using the shortcut alone. Opening
   `/subagents` refreshed a picker with full root/child UUIDs and current marker.
4. Selecting the second row with `down enter` displayed the child in the same
   pane. `alt+left` returned to the main conversation, confirmed by main footer.
5. Injecting `/subagents` into the read-only child left composer text that prevented
   navigation. Several editing shortcuts did not clear it. The test process was
   exited and the synthetic parent resumed. Production must never use that path.
6. Codex source confirms navigation only with empty composer and no popup/modal.
7. Herdr 0.9.5 rejects the logical key `end`. Use installed help and capability
   tests rather than assuming every modern website example applies.

## Conclusion

Switching the existing official Codex TUI through Herdr is feasible on Windows.
A panel can translate a click into such navigation, but the reliable arbitrary
target selector, mouse hit testing, safe UI-state checks, long/duplicate-name
lists, active-turn behavior and Linux execution remain unproven. No public
direct external select-thread RPC was established. Do not advertise unconditional
one-click switching until task 1/4 acceptance gates pass.

The Rust bridge is a small reproducible transport spike. It is not a mouse UI,
does not resolve thread IDs, and must only target isolated test panes.

The dependency-free `navigation.rs` compiled successfully with Rust 1.99.0.
Running its `next` action displayed the child's read-only view; its `previous`
action displayed `Main [default]` in the same pane. Both processes exited zero.
This validates Rust-to-Herdr key delivery, not arbitrary-target verification.
