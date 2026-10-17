---
title: Pi
description: What the sidebar shows for Pi panes, and how the extension bridge maps its events.
---

Pi works with the sidebar through a local TypeScript extension bridge, so the
visible surface is similar to OpenCode but sourced from Pi's extension event
stream.

## What you get

### Status and prompts

- Live status from `session_start` / `agent_start` / `agent_end` / `agent_settled`
- Prompt text from `before_agent_start` (the run's user prompt)
- Status returns to `running` from `ui_prompt_end` after a blocking prompt closes
- Elapsed time since the last prompt

### Attention cues

- Waiting status + wait reason from `ui_prompt_start`
- Error status from `turn_end` (outcome `error`) and `session_compact_failed`
- `notification` desktop alerts for blocking prompts

### Activity log

- Tool calls recorded from `tool_execution_start`

### Git

- Branch display from the pane's `cwd`

## What is not available

| Feature                    | Why |
| -------------------------- | --- |
| Response preview (`▷ ...`) | The bridge does not forward final assistant text on `agent_end` |
| Permission badge           | Pi does not expose the Claude-style permission modes |
| Background shell state     | Pi does not emit a background Bash flag |
| Task progress counter      | The bridge does not map a task-progress event |
| Sub-agent tree             | Pi does not emit Claude-style sub-agent hooks |
| Worktree lifecycle tracking | Pi does not emit `WorktreeCreate` / `WorktreeRemove` |

## Setup

Wire the extension bridge from [Pi setup](/tmux-agent-sidebar/getting-started/pi/).
