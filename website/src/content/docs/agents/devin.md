---
title: Devin CLI
description: What the sidebar shows for Devin CLI panes, and what is not available due to the Devin hook schema.
---

Devin CLI's hooks are modeled on Claude Code's hook format, so the visible
surface is similar to Claude Code but narrower — Devin CLI does not have
Claude-style sub-agents, and does not expose permission modes or worktree
lifecycle hooks.

## What you get

### Status and prompts

- Live status from `SessionStart` / `UserPromptSubmit` / `Stop`
- Prompt text from `UserPromptSubmit`
- Elapsed time since the last prompt

### Attention cues

- Waiting status from `PermissionRequest` (mapped to the sidebar's
  permission-denied event kind — Devin fires this when a permission
  decision is needed, not only on denial)

### Activity log

- Tool calls recorded from `PostToolUse`

### Git

- Branch display from the pane's `cwd`

## What is not available

| Feature                    | Why |
| --------------------------- | --- |
| Permission badge            | Devin CLI's hook payload does not include a permission-mode field |
| Response text display       | Devin's `Stop` payload does not carry the final assistant message |
| Background shell state      | Devin's `PostToolUse` payload does not document a background-shell flag |
| Task progress counter       | Devin CLI does not emit task lifecycle events |
| Sub-agent tree               | Devin CLI does not emit `SubagentStart` / `SubagentStop` |
| Worktree lifecycle tracking | Devin CLI does not emit `WorktreeCreate` / `WorktreeRemove` |
| Compaction notice           | Devin's `PostCompaction` has no sidebar equivalent yet |

## Setup

Wire the hooks from inside a Devin CLI pane — see [Devin CLI setup](/tmux-agent-sidebar/getting-started/devin/).
