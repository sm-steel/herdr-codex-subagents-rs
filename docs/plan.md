# Herdr Codex Subagents Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans for inline implementation,
> or superpowers:subagent-driven-development if the maintainer selects delegation.
> Execute task-by-task only after design review. Checkboxes track actual completion.

**Goal:** One Windows/Linux Herdr panel listing main and subagents, their status,
task and activity, with verified navigation in the existing Codex pane.

**Architecture:** Read-only versioned rollout adapter feeds a synchronous typed
reducer. Ratatui renders one panel. A separate guarded navigation controller calls
Herdr's CLI; observation never submits prompts or changes Codex configuration.

**Tech Stack:** Rust 2024, Ratatui/Crossterm, serde/serde_json, clap, tracing;
tempfile/assert_cmd/proptest as needed. Resolve and approve exact dependency
versions during foundation; no product Cargo project exists yet.

**Spec:** [design.md](design.md). Rules: [rust-rules.md](rust-rules.md).

## Execution scope update

Monitoring-only preview is authorized. Task 4 and click navigation are deferred.
The Rust core, incremental observation, CLI, Herdr manifest, panel and CI are
implemented on feat/panel. See compatibility.md for tested environments and known
gaps. Windows/Linux CI, independent review and final smoke checks decide readiness.

## Global constraints

- Windows and Linux required; Herdr 0.9.5 / Codex 0.166.0 initial baseline.
- Main first, stable descendant ordering, status icon and text on every row.
- Preserve drafts, approvals, pane ownership and the original Codex process.
- No model calls, credentials, telemetry, arbitrary focus targeting, shell scripts,
  or raw conversation text in logs.
- Proposal: Rust 1.99.0 toolchain, MSRV 1.97, `#![forbid(unsafe_code)]`.
- Strict quality/test policies in rust-rules.md; no weakened approved tests.

## Review focus

1. Same workspace has two sessions: resolve exact root, never newest-by-cwd (task 2).
2. Draft/approval/user keystroke races: never clear or accidentally submit (task 4).
3. Narrow picker/duplicate labels: exact UUID selection or refuse (task 4).
4. Partial UTF-8/truncated/oversize rollout: bounded recovery, no false completion (task 3).
5. Quiet main with active children: own status separate from rollup (task 3).

## Task 1: Complete navigation feasibility and portability gate

**Files:** `poc/navigation.rs`, `poc/RESULTS.md`, `docs/design.md`.
**Consumes:** explicit Herdr binary/session/pane and synthetic isolated Codex threads.
**Produces:** capability matrix and versioned evidence, not a production selector.

- [x] Windows: spawn a single real child; select it in native picker and return to main.
- [x] Compile and exercise Rust bridge against isolated pane; record result.
- [ ] Test exact selection among two children including duplicate display names,
  a resumed root, a closed child and a narrow pane. Record failure behavior.
- [ ] Test draft and approval preservation, active turn navigation, user input race,
  and manual thread changes. Do not clear input in production code.
- [ ] Prove input exclusion or atomic TUI navigation before claiming preservation
  under concurrent typing/approval arrival. Without it, safe one-click switching
  is blocked; screen checks alone cannot satisfy the acceptance contract.
- [ ] Repeat on Linux; mark unavailable infrastructure explicitly, never infer pass.
- [ ] If exact click navigation cannot be verified, stop and revise design before
  implementing a misleading selector. Keep native picker fallback explicit.
- [ ] Commit findings as `docs: record thread navigation compatibility`.

## Task 2: Foundation, plugin integration and exact source discovery

**Files:** `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `clippy.toml`,
`deny.toml`, `mise.toml`, `lefthook.yml`, `herdr-plugin.toml`,
`src/{lib,main,session,herdr}.rs`, `tests/it/{main,discovery,herdr}.rs`.
**Interfaces:** `ThreadId`, `PaneId`, `SourceContext::resolve(EnvSnapshot,
InvocationContext) -> Result<SourceContext>`; `HerdrClient` owns process calls.

- [ ] Add failing integration cases for two same-cwd sessions, missing identity,
  viewer-versus-parent context, Unicode/spaced paths, stale pane replacement.
- [ ] Implement one library+binary package and minimal explicit `doctor`, `open`,
  `watch --root-thread <id> --target-pane <id>` commands. No implicit newest root.
- [ ] Pin audited dependencies and tools; adapt lint/cargo-deny policies as specified.
- [ ] Characterize actual Herdr invocation/session response shapes with local CLI.
  Fake the executable with a Rust test helper to capture exact argv and env.
- [ ] Add per-OS manifest build/pane entries; verify using `herdr plugin link` on
  a named test server. An action opens exactly one no-focus panel beside parent.
- [ ] Run fmt, strict Clippy, nextest and cargo-deny. Commit foundation.

## Task 3: Incremental reader and agent/status model

**Files:** `src/agent.rs`, `src/rollout.rs`, `src/rollout/{record,tail,index}.rs`,
`tests/it/{rollout,recovery}.rs`, `tests/fixtures/codex-0.166.0/`.
**Interfaces:** `ObservedEvent` carries thread/call identity and provenance;
`AgentTree::apply(event, observed_at)`; `snapshot(now) -> AgentSnapshot`;
`RolloutReader::poll() -> Result<Vec<ObservedEvent>>`.

- [ ] Characterize v1/v2 upstream shapes using sanitized synthetic fixtures; record
  mapping tables for lifecycle, delegation, messages, plans and tool activity.
- [ ] Write reducer tests failing on assertions; obtain maintainer review before
  implementation. Assert main-first order, exact parent relations, interrupted vs
  completed, waiting vs working, idle-main/working-children and stale provenance.
- [ ] Implement typed state transitions without I/O or ambient time; define what
  a repeated/new turn does to completed state and deduplicate events by identity.
- [ ] Test appends split at every UTF-8/JSON boundary, CRLF, missing final newline,
  replacement, shrink, unknown variants, oversize record and late child creation.
- [ ] Implement bounded tails, offsets and backoff from design; skip oversized
  records until next newline. No duplicate activity on reconciliation/restart.
- [ ] Verify stable output and bounds with property tests and large generated logs;
  no real user sessions in fixtures. Commit reader and reducer.

## Task 4: Verified click-to-switch controller

**Files:** `src/navigation.rs`, `src/navigation/{picker,verification}.rs`,
`tests/it/navigation.rs`, `tests/fixtures/codex-0.166.0/navigation/`.
**Interfaces:** `request_switch(TargetThreadId) -> Result<NavigationOutcome>`;
outcomes distinguish verified success, already selected, unsafe UI, unsupported
version and unverifiable target. Uses task 2 client and task 3 identities.

- [ ] Write fake-Herdr transcript cases for exact UUID, duplicate labels, truncation,
  empty/long lists, popup, draft, approval, closed/replaced pane, user typing race,
  timeout, stale row order and manual navigation. Assert no prompt submission.
- [ ] Implement serialized read-check-navigate-verify with bounded retries and
  explicit target pane. A successful subprocess write is not navigation success.
- [ ] Never clear text, dismiss approval, restart Codex, or operate on focused pane.
  Stop after unexpected screen state; show native picker fallback if unsupported.
- [ ] Repeat real root/two-child round trip in both OSs, proving same process/pane
  and unchanged drafts. Commit controller only after capability gate passes.

## Task 5: Compact mouse/keyboard panel

**Files:** `src/panel.rs`, `src/panel/{input,render}.rs`, `src/diagnostics.rs`,
`tests/it/panel.rs`; scoped inline tests beside each module.
**Interfaces:** `Panel::update(AgentSnapshot)`, `Panel::handle(Input) -> Action`;
`Action::Select(ThreadId)` uses stable row hit regions, not display names.

- [ ] Test main always top, tree indent, scroll/resize, Unicode width, mouse row
  hit testing, keyboard Enter, empty states and stale-source labels.
- [ ] Render per-row working spinner plus text/ASCII fallback, waiting, completed,
  failed, interrupted, closed and unknown. Main own state and child count differ.
- [ ] Highlight active thread only on verified navigation; retain completed rows.
- [ ] Sanitize ANSI/OSC/control characters; test text cannot emit terminal commands.
- [ ] Add bounded worker lifecycle, poll cadence, terminal RAII restoration and
  clean q/Ctrl-C exit. Logs use a separate sink and never raw conversation text.
- [ ] Run real mouse navigation on Windows/Linux. Commit panel.

## Task 6: CI, packaging and release readiness

**Files:** `.github/workflows/{ci,release}.yml`, `README.md`, `docs/compatibility.md`,
`SECURITY.md`, license files and release packaging configuration.
**Produces:** tested installable Windows/Linux artifacts and honest support matrix.

- [ ] Add Windows x64/Linux x64 jobs, plus Linux ARM64 if claimed. Validate MSRV,
  format, strict Clippy, unit/integration/doc tests, cargo-deny and unused deps.
- [ ] Pin actions by SHA and tools by version; minimal permissions; verify manifests,
  gitleaks, actionlint and zizmor. Secrets never enter fork PR runs.
- [ ] Build locked release archives with manifest, binary and checksums. Test clean
  install/link/update/uninstall without touching an existing Codex session.
- [ ] Document requirements, invocation, keybindings, data sensitivity, compatibility
  limits and recovery. Add actual smoke-test evidence, not only cross-compilation.
- [ ] Only after checks pass, prepare release for maintainer review; public repository
  creation is authorized, publishing a functional release is a later decision.

## Handoff

The repository contains planning and a throwaway spike only. Review the design,
especially screen-driven navigation limits, before implementing these tasks.
