---
title: Cursor
description: What the sidebar shows for Cursor CLI panes, and what the CLI's hook gaps leave out.
---

Cursor CLI (`agent`) exposes the smallest usable hook set of the four supported
agents — not because Cursor documents few hooks, but because most of the
documented ones do not fire in the terminal client yet.

## What you get

### Status and prompts

- Live status from `sessionStart` / `stop`, plus a `running` flip on the first tool call of a turn
- Error status when a turn ends with `status: "error"`
- Elapsed time since the turn started

### Git

- Branch display from the pane's `cwd`
- PR number (needs `gh` CLI)

### Notifications

- `stop` only — fires when the assistant finishes responding.

### Activity log

- Every tool Cursor reports through `postToolUse`. `Shell` is normalised to `Bash` and `Task` to `Agent` so entries share one vocabulary with the other agents; MCP calls arrive as `MCP:<tool>` and are rewritten to `mcp__<tool>`.

## What is not available

| Feature                          | Why                                                                                              |
| -------------------------------- | ------------------------------------------------------------------------------------------------ |
| Prompt text display              | Needs `beforeSubmitPrompt`, which the CLI does not fire                                          |
| Response preview (`▷ …`)         | Needs `afterAgentResponse`, which the CLI does not fire; Cursor's `stop` payload carries no message |
| Waiting status + wait reason     | Cursor's only permission hooks are blocking gates that decide whether a call proceeds — the sidebar stays out of that path |
| Background shell state           | Cursor has no background-Bash flag                                                               |
| Permission badge                 | No hook reports the active permission mode                                                       |
| Task progress counter            | Cursor has no task tool for the activity log to count                                            |
| Sub-agent tree                   | `subagentStart` / `subagentStop` are documented but not confirmed in the CLI                     |
| Worktree lifecycle tracking      | Needs `WorktreeCreate` / `WorktreeRemove` (Claude-only)                                          |

## Teardown

Cursor's `sessionEnd` is wired, but it is not confirmed to fire in the CLI, so
the sidebar also sweeps Cursor panes the way it does Codex and OpenCode: when
tmux reports a plain shell as the pane command and no `agent` process is left in
the pane's process tree, the pane's metadata and activity log are cleared on the
next poll.

## Spawning

`n` in the sidebar can launch Cursor into a fresh worktree. Two modes are
offered: `default`, and `bypassPermissions` which adds `--force`. The spawn
command is `agent` — the executable Cursor installs — not `cursor`.

## Setup

Wire the hooks from inside a Cursor pane — see [Cursor setup](/tmux-agent-sidebar/getting-started/cursor/).
