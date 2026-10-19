---
title: omp
description: What the sidebar shows for omp (oh-my-pi) panes, and how the extension bridge maps its events.
---

omp ([oh-my-pi](https://github.com/can1357/oh-my-pi)) works with the sidebar
through a local extension bridge, so the visible surface is similar to
OpenCode but with a different event source.

## What you get

### Status and prompts

- Live status from `session_start` / `before_agent_start` / `agent_start` / `agent_end`
- Prompt text from `before_agent_start`
- Response preview (`▷ ...`) from the last assistant message on `agent_end`
- Elapsed time since the last prompt

omp has no "settled" event, so `agent_end` is debounced (250 ms) before the
pane goes idle. A new `agent_start` inside that window — an automatic retry,
a queued follow-up — cancels the pending idle.

### Attention cues

- Waiting status from `tool_approval_requested` (wait reason `permission`)
  and from the `ask` tool (wait reason `elicitation_dialog`). The next tool
  result or the end of the turn moves the pane back out of `waiting`.
- API failure reason from an assistant message that ends with
  `stopReason: "error"`. Retryable provider errors (rate limits, overloads,
  timeouts) are held for 2.5 s so omp's own auto-retry can start a new turn;
  the pane only turns red if none does.

### Activity log

- Tool calls recorded from `tool_result`. `bash`, `read`, `write`, `edit`,
  `grep`, `glob`, `web_search`, `task`, and `todo` are normalized into the
  same vocabulary Claude Code uses; other tools are still logged but without
  a derived label. omp's `edit` takes hashline text with no `path`, so edit
  rows have no file label.

### Git

- Branch display from the pane's `cwd`

## Subagents

omp loads extensions again inside every subagent session, and subagents
inherit the parent's `$TMUX_PANE`. The bridge ignores any session that is not
the interactive root session, so subagent activity never overwrites the
parent pane's prompt or status. The parent's own `task` tool call still shows
up in the activity log.

## What is not available

| Feature                      | Why                                                       |
| ---------------------------- | --------------------------------------------------------- |
| Permission badge             | omp does not expose Claude-style permission modes         |
| Background shell state       | omp does not currently document a background Bash flag    |
| Task progress counter        | The bridge does not map a task-progress event             |
| Sub-agent tree               | Subagent sessions are intentionally ignored (see above)   |
| Worktree lifecycle tracking  | omp does not emit `WorktreeCreate` / `WorktreeRemove`     |

## Setup

Wire the extension bridge from [omp setup](/tmux-agent-sidebar/getting-started/omp/).
