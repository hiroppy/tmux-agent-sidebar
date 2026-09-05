---
title: Grok Build
description: How Grok Build hooks map to sidebar status, permissions, subagents, and activity.
---

Grok Build exposes a broad hook lifecycle. The sidebar reads Grok's camelCase payloads and keeps turn completion correlated by `promptId`, so a delayed `StopCancelled` from an older turn cannot settle a newer prompt.

## What you get

### Status and prompts

- Live status from `SessionStart`, `UserPromptSubmit`, `Stop`, `StopFailure`, and `StopCancelled`
- Prompt and response text from `UserPromptSubmit` and `Stop`
- Waiting status from `Notification` with the `permission_prompt` matcher
- Idle recovery from `Notification` with the `idle_prompt` matcher
- API failure reason from `StopFailure`
- Elapsed time since the current prompt

### Permissions and notifications

- Permission badges for `plan`, `auto`, and `!`
- Desktop notifications for completed turns, permission prompts, denied permissions, and failures

### Subagents and activity

- Subagent display from `SubagentStart` / `SubagentStop`
- Background subagents remain visible after the parent turn settles
- Activity entries from `PostToolUse` / `PostToolUseFailure`
- Native tools normalized to sidebar categories, including terminal, read, edit, grep, file listing, web search, and subagent work

### Git and worktree spawning

- Branch display from the pane's `cwd`
- PR number when `gh` is available
- Grok can be selected in the worktree spawn modal with any supported `--permission-mode`

## What is not available

| Feature | Why |
| ------- | --- |
| Task progress counter | Grok does not expose the sidebar's task-list tool lifecycle |
| Agent-created worktree lifecycle | Grok does not expose `WorktreeCreate` / `WorktreeRemove`; sidebar-created worktrees still work |

## Blocking Stop hooks

The sidebar's `Stop` observer provides immediate completion updates. Grok cannot tell a passive observer whether a later `Stop` hook will block that same stop, so a separate blocking `Stop` hook can briefly make the sidebar look idle while Grok continues. When using a blocking stop gate, remove the sidebar's `Stop` registration and keep its `StopFailure`, `StopCancelled`, `idle_prompt`, and `SessionEnd` registrations; the idle backstop will settle the pane after Grok actually stops.

## Setup

Install the dedicated hook file from [Grok Build setup](/tmux-agent-sidebar/getting-started/grok/).
