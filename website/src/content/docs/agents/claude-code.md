---
title: Claude Code
description: Every sidebar feature that lights up with Claude Code hooks.
---

Claude Code is the reference agent for the sidebar — every feature is wired through a Claude hook.

## What you get

### Status and prompts

- Live status (`running` / `waiting` / `idle` / `error`) from `SessionStart` / `UserPromptSubmit` / `Stop`
- Prompt text from `UserPromptSubmit`
- Response preview (`▷ …`) from `Stop`
- Elapsed time since the last prompt

### Attention cues

- Waiting status + wait reason from `Notification`, `PermissionDenied`, `TeammateIdle`
- API failure reason from `StopFailure`
- Permission badges: `plan`, `edit`, `auto`, `dontAsk`, `defer`, `!`

### Work-in-progress view

- Task progress counter (e.g. `3/7`) — requires `PostToolUse`
- Sub-agent tree — requires `SubagentStart` / `SubagentStop`
- Activity log — every tool call recorded via `PostToolUse`

### Git and worktrees

- Branch display, updated dynamically via `CwdChanged`
- Worktree lifecycle tracking via `WorktreeCreate` / `WorktreeRemove`
- PR number (needs `gh` CLI)

### Notifications

Every desktop notification event is available — `stop`, `notification`, `task_completed`, `stop_failure`, `permission_denied`. See [Notifications](/tmux-agent-sidebar/features/notifications/).

## Known limitation

**Waiting status** — after you approve a permission prompt, the status stays `waiting` until the next hook event fires. This is a limitation of the Claude Code hook system.

## Setup

Install the plugin — see [Claude Code setup](/tmux-agent-sidebar/getting-started/claude-code/).
