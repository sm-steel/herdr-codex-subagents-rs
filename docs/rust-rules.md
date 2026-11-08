# Rust and engineering rules

Adapted selectively from semoxide; migration rationale is in
[rule-migration.md](rule-migration.md). These apply to product code; the explicitly
throwaway navigation spike does not establish the product architecture.

## Language and structure

1. No `unwrap`, `expect`, or `panic!` outside tests.
2. Use `#[expect(lint, reason = "...")]`, never blanket `#[allow]`.
3. Encode meaning in validated newtypes and enums; no ambiguous boolean arguments.
   Use `Option` for actual optional data, including absent upstream metadata.
4. Private struct fields, constructors/builders, narrow `pub(crate)` visibility.
5. Conversions use `From`, `TryFrom`, `FromStr`; fallible conversions never `From`.
   Names follow `as_`/`to_`/`into_`; getters have no `get_` prefix.
6. Every crate root has `#![forbid(unsafe_code)]`; no semoxide FFI exception.
7. Prefer stable modern std over dependencies; use only features supported by MSRV.
8. Flat control flow with `?`, early returns, and shallow nesting.
9. One domain per file; `foo.rs` and `foo/child.rs`, no `mod.rs`. Split by concept,
   not arbitrary length; no tiny speculative crates.
10. Concrete types until a real second implementation justifies a trait.
11. Pure reducer accepts typed data and explicit time; filesystem, environment,
    terminal, subprocesses, and wall clocks stay in adapters.
12. Thin CLI calling library APIs; typed errors are rendered only at the edge.
    Use `#[non_exhaustive]` on public errors/types where future extension is likely.
13. Explicit deterministic order; do not render from hash collection iteration.
14. Short informative comments; public API docs describe behavior and invariants.
    No conversational history in code comments or reference documentation.

## Runtime and diagnostics

- Snapshot environment once (`OsString` values); Windows names compare ASCII
  case-insensitively. Library code never reads or mutates process environment.
- Herdr subprocess spawning has one owning adapter, argv only, bounded output,
  deadline, cancellation, and wait/reaping. No hidden shell or leaked workers.
- Child environments use `env_clear()` plus a documented allowlist derived from
  the snapshot. Preserve Herdr connection context, required Windows system vars,
  PATH and relevant home/config vars; exclude API keys and unrelated secrets.
- Synchronous pure core. Prefer a bounded worker and std channels initially.
  Tokio requires a measured need and design review, not a blanket ban.
- Cancellation is cooperative; join workers, restore terminal via RAII, and never
  terminate another process merely because observation or navigation failed.
- Library emits `tracing` events but installs no subscriber and prints nothing.
  CLI owns subscriber/output. Diagnostics never contain raw conversation content.
- Structured stdout is data only in JSON commands; TUI rendering owns its terminal.
  Logs use a separate sink while TUI runs. Handle broken pipes without panicking.

## Tooling policy to implement

- Rust 2024; exact stable toolchain pin. Proposed baseline from semoxide:
  toolchain 1.99.0, MSRV 1.97; validate dependencies and both target OSs first.
- Centralize exact dependency versions; commit Cargo.lock; use `--locked` in CI.
  Audit new crates and features; avoid speculative dependencies.
- Pedantic Clippy plus selected restrictions: unwrap/expect/panic, indexing,
  stdout/stderr, dbg/todo/unimplemented, fallible From, module layout, clone
  hygiene, hash iteration, use_self, private-field conventions.
- Clippy warnings become errors in CI. Complexity <=25, nesting <=5; keep normal
  Clippy function-size/argument defaults. Do not loosen rules to pass checks.
- Ban ambient env reads, direct process spawning, and printing outside owning
  adapters with reasoned local expectations. Tests may unwrap/assert/index.
- cargo-deny: no ignored advisories by default, deny yanked/wildcard/unknown-source
  dependencies, permissive-license allowlist. No semoxide-specific crate bans.
- Pin tools in mise; use rustfmt, Clippy, nextest, cargo-deny, cargo-shear, typos,
  gitleaks, actionlint, zizmor, markdownlint and lefthook where introduced.
  Add property/fuzz/mutation checks to parser/reducer risks, not every trivial helper.
- CI actions pin commit SHAs, minimal permissions, Windows/Linux jobs; no secrets
  in fork PR jobs. No meaningless product-code tests for docs-only foundation.
- Conventional Commits; branch work; never bypass hooks with `--no-verify`.

## Tests

- Pure-domain behavior: spec/table -> tests failing on assertions -> maintainer
  review -> implementation. No compile-error or `todo!()` substitute for real red.
- Never weaken approved tests to pass; obtain maintainer review for a separate
  `test:` change. Report existing test changes in PR descriptions.
- Inline unit-test module is last in its source file. Integration suite has one
  `tests/it/main.rs` binary. Fixtures are synthetic and contain no private sessions.
- Prefer executable fakes and real temp files/processes over mocking frameworks.
- New/changed insta snapshots remain pending human acceptance; no automatic accept.
- Every bug receives a failing regression case. Inject time and file events;
  no sleeping unit tests. Async tests, if needed, use paused deterministic time.
- Test UTF-8/CRLF, unknown JSON fields, partial records, truncation, ANSI/OSC
  injection, duplicate labels, stale state, Windows paths and exact thread identity.
