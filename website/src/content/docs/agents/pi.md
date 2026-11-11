---
title: pi
description: What the sidebar shows for pi panes, and how the extension bridge maps its events.
---

pi works with the sidebar through a local extension bridge, so the visible
surface is similar to OpenCode but with a different event source.

## What you get

### Status and prompts

- Live status from `session_start` / `before_agent_start` / `agent_settled`
- Prompt text from `before_agent_start`
- Response preview (`▷ ...`) from the last assistant message, flushed on `agent_settled`
- Elapsed time since the last prompt

### Activity log

- Tool calls recorded from `tool_result` — `bash`, `read`, `write`, `edit`, and
  `powershell` are normalized into the same vocabulary Claude Code uses;
  other tools are still logged but without a derived label

### Git

- Branch display from the pane's `cwd`

## What is not available

| Feature                     | Why                                                                                     |
| ---------------------------- | ---------------------------------------------------------------------------------------- |
| Permission badge             | pi does not expose Claude-style permission modes to extensions                          |
| Waiting status + wait reason | pi has no native permission-approval event exposed to extensions                        |
| Background shell state       | pi's bash tool result does not currently document a background flag                      |
| API failure reason           | pi's extension API has no dedicated "agent errored" lifecycle event distinct from `stop` |
| Task progress counter        | The bridge does not map a task-progress event                                            |
| Sub-agent tree               | pi does not emit Claude-style sub-agent lifecycle events to extensions                  |
| Worktree lifecycle tracking  | pi does not emit `WorktreeCreate` / `WorktreeRemove`                                     |

## Setup

Wire the extension bridge from [pi setup](/tmux-agent-sidebar/getting-started/pi/).
