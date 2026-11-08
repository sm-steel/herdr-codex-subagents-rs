# Using Codex Subagents

The monitor is read-only. It expands and folds its own tree; selecting an agent
does not switch threads or send input to Codex. No API key, model request,
authentication change or telemetry is involved.

## The tree

Main stays pinned first and initially expanded. Nested branches initially fold.
Siblings sort by working first, then their latest observed rollout update;
parents include descendant activity. Equal times preserve stable ordering.

A four-row Braille icon animates when the agent or any descendant works. Other
agents show a gray ⣿. Main's right-aligned `[N of X]` counts descendants whose own
recorded state is working, out of all descendants, including folded ones.

Task is the assignment; Last is the latest observed activity. A source that becomes
unreadable is marked stale. Recorded lifecycle is not proof of process liveness:
quiet files never imply completion. Last-active sorting follows local rollout
modification times, so copying or restoring logs can change their order.

## Controls

| Input | Action |
| --- | --- |
| Up / Down | Select a row |
| Left | Fold the branch, or select its parent |
| Right | Expand the branch, or select its first child |
| Enter / Space / click | Toggle the selected branch |
| Home / End | Select the first / last visible row |
| q / Ctrl-C | Close only the monitor |

▶ / ▼ mark foldable branches; the dim ─ marks agents without children. Selection
follows the same agent when sorting changes. Hidden descendants continue to update
icons and counts. Main remains pinned while children scroll.

## Automatic attachment

The trusted SessionStart hook supplies the exact Codex session identity. An
existing `codex resume <UUID>` invocation can also be identified without a restart.
A fresh running session without either identity stays untouched until the hook
reports it. No session is guessed from its folder or age.

Startup, agent detection, status changes and pane moves reconcile monitors.
Repeated events reuse the same panel. Moving a source can leave its existing
monitor in its original tab. An ambiguous creation failure suppresses retries
instead of opening duplicate panes; plugin command logs identify skipped work.

## Troubleshooting

If no panel appears, check `herdr plugin list`, then review the hook in Codex
`/hooks` and start or resume a session. Existing sessions that predate the hook
may not have an exact identity. Run the **Attach Codex monitors automatically**
action after an installation into an already-running server.

A working icon describes the recorded state, including active descendants. It
can remain working when a process stopped without recording a terminal event.
Unknown data remains unknown. The Codex rollout format is internal and may change.

## Explicit observation

Advanced diagnostics can run the binary directly:

```sh
herdr-codex-subagents-rs doctor
herdr-codex-subagents-rs watch --root-thread <UUID> --once
```

`--once` emits JSON with distinct lifecycle outcomes. `--sessions-dir <path>`
selects a fixture or custom log directory; defaults are `CODEX_HOME/sessions` or
`~/.codex/sessions`. Use the binary path printed by the installer if it is not on
PATH. SSH observation runs on the machine owning the session files.

See [installation](install.md), [compatibility](compatibility.md) and
[release packaging](packaging.md).
