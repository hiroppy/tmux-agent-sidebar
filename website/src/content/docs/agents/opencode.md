---
title: OpenCode
description: What the sidebar shows for OpenCode panes, and how the plugin bridge maps its events.
---

OpenCode works with the sidebar through a local plugin bridge, so the visible
surface is similar to Codex but with a different event source.

## What you get

### Status and prompts

- Live status from `session.created` / `session.status` / `session.idle`
- Prompt text from `session.status=active`
- Response preview (`▷ ...`) from `stop`
- Elapsed time since the last prompt

### Attention cues

- Waiting status + wait reason from `permission.asked`
- API failure reason from `session.error` / `session.status=error`
- `notification` desktop alerts for permission prompts

### Activity log

- Tool calls recorded from `tool.execute.after`

### Git

- Branch display from the pane's `cwd`

## What is not available

| Feature                    | Why |
| -------------------------- | --- |
| Permission badge           | OpenCode does not expose the Claude-style permission modes |
| Background shell state     | OpenCode does not currently document a background Bash flag |
| Task progress counter      | The bridge does not map a task-progress event |
| Sub-agent tree             | OpenCode does not emit Claude-style sub-agent hooks |
| Worktree lifecycle tracking | OpenCode does not emit `WorktreeCreate` / `WorktreeRemove` |

## Setup

Wire the plugin bridge from [OpenCode setup](/tmux-agent-sidebar/getting-started/opencode/).
