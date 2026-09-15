---
title: GitHub Copilot CLI
description: What the sidebar shows for GitHub Copilot CLI panes, and what is not available due to the Copilot hook schema.
---

GitHub Copilot CLI exposes a hook system with its own config shape and
camelCase event/field names, so the visible surface is similar to Codex.

## What you get

### Status and prompts

- Live status from `sessionStart` / `userPromptSubmitted` / `agentStop`
- Prompt text from `userPromptSubmitted`
- Elapsed time since the last prompt

### Activity log

- Tool calls recorded from `postToolUse`

### Git

- Branch display from the pane's `cwd`

## What is not available

| Feature                     | Why |
| ---------------------------- | --- |
| Permission badge             | Copilot CLI's hook payload does not include a permission-mode field |
| Response text display        | Copilot's `agentStop` payload carries a transcript path, not the final response text |
| Waiting status + wait reason | Needs `permissionRequest` / `notification` (not wired — Copilot's own permission flow already handles these) |
| Background shell state       | Copilot's tool-call payload does not document a background-shell flag |
| Task progress counter        | Copilot CLI does not emit task lifecycle events |
| Sub-agent tree                | Copilot CLI's `subagentStart` / `subagentStop` are not wired |
| Worktree lifecycle tracking  | Copilot CLI does not emit `WorktreeCreate` / `WorktreeRemove` |

## Setup

Wire the hooks from inside a Copilot CLI pane — see [Copilot CLI setup](/tmux-agent-sidebar/getting-started/copilot/).
