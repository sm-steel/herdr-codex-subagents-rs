# Security

This is a local read-only monitor, not an agent-control or credential tool.
It reads Codex session logs, so it has access to sensitive conversation text under
your OS user's permissions. No model requests, analytics, or external network
connections are part of normal operation.

Do not attach real session rollouts, prompts, tool output, or credentials to public
issues. Supply a minimal synthetic reproduction and version numbers instead.
For a private security report, use the repository's GitHub security reporting
feature if enabled; otherwise ask for a private contact without posting details.

The executable trusts `HERDR_BIN_PATH` as host configuration. Use a trusted Herdr
installation and plugin checkout. Plugin actions can create the monitor pane but
cannot submit prompts, approve requests, switch threads, or stop Codex agents.
