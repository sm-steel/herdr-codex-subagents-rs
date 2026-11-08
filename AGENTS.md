# Project instructions

Read [docs/design.md](docs/design.md), [docs/rust-rules.md](docs/rust-rules.md), and
the task in [docs/plan.md](docs/plan.md) before implementation.

- Work on a branch after the initial repository import; Conventional Commits;
  keep the global Git identity.
- Research and capability spikes precede assumptions about external protocols.
- Never weaken approved tests, lint rules, thresholds, or allowlists to pass.
- New snapshots remain pending human review; never accept them automatically.
- Pure-core tests must fail on assertions and receive maintainer review before
  implementation. I/O boundaries use characterization tests after a spike.
- Never navigate, submit text to, or close an arbitrary focused pane. Resolve an
  explicit parent pane and verify its live Codex identity first.
- No model API calls or credentials are needed by this project.
- Documents describe the final contract, stay short, and link instead of repeating.
- Do not implement product code until the design is approved. The `poc/` spike
  is explicitly authorized exploratory code, not a production foundation.
