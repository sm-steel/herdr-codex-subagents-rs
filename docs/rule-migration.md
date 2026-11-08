# Semoxide rule migration

Read sources: `AGENTS.md`, `CLAUDE.md`, `docs/CODE-ARCHITECTURE.md`,
`docs/RUST-PITFALLS.md`, `docs/TESTING.md`, `docs/OBSERVABILITY.md`,
`.claude/skills/rust-testing/SKILL.md`, `.claude/skills/rust-async/SKILL.md`,
`Cargo.toml`, `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, `deny.toml`,
and `mise.toml` in the user's semoxide project. Source projects are not modified.

| Treatment | Policies and reason |
| --- | --- |
| Retain | Eight core Rust rules, typed errors, private fields, narrow modules, pure core, RAII, deterministic output, concise docs, environment snapshot, tracing boundaries. All apply to a local monitor. |
| Retain | Test-first pure core with human review, approved-test preservation, pending snapshots, inline unit tests, one integration binary, fakes instead of mocking frameworks. |
| Retain | Strict lints/thresholds, pinned tools/deps, lockfile, supply-chain/license checks, branch workflow, Conventional Commits, no hook bypass, no custom commit email. |
| Adapt | Single library+binary package instead of eight crates. Split later only for real dependency/purity boundaries. |
| Adapt | Optional metadata remains Option; avoid interpreting the type rule as banning legitimate absence. |
| Adapt | No unsafe at all; semoxide's libgit2 transport exception does not apply. |
| Adapt | Process owner is Herdr adapter, environment allowlist includes Windows/Herdr necessities; output owner is terminal renderer. |
| Adapt | Sync reducer with bounded worker lifecycle, rather than semoxide's exact async/gRPC/SSH topology. |
| Adapt | Diagnostic metadata only: do not reproduce the full secret-registry machinery or log raw prompts for debugging. |
| Adapt | Toolchain 1.99.0/MSRV 1.97 are proposed pins, not a claim of compatibility before CI. Pin other tools when foundation is implemented. |
| Defer | Mutation/fuzz gates, API semver checks, benchmarks and dependency-count budget until meaningful code/baselines exist. |
| Omit | Release pipeline, rollback/remote-write tracking, git2-only rules, npm/GitHub publishing semantics, forge credentials, semantic-release compatibility and golden Git histories. |
| Omit | gRPC plugin SDK, schema/error/façade crate proliferation, plugin secret registration, git-specific license exceptions and dependency wrapper bans. |
| Omit | Dogfooding a release tool to release itself; this project instead dogfoods the monitor against real Codex sessions. |

Behavioral rules are consolidated in `rust-rules.md` instead of copying project-
specific skill files whose scope is semoxide. Existing numeric quality thresholds
are retained; no claims that lint enforcement exists before Cargo/CI is added.
