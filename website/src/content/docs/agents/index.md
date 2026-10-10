---
title: Agent support overview
description: What the sidebar shows for Claude Code, Codex, OpenCode, and Oh My Pi (OMP), side by side.
---

Claude Code, Codex, OpenCode, and OMP work with the sidebar, but they expose different sets of hooks — so the sidebar's surface area is narrower for Codex, OpenCode, and OMP than it is for Claude Code.

## Feature support by agent

| Feature                                  | Claude Code | Codex        | OpenCode     | OMP          | Notes                                                                                                                           |
| ---------------------------------------- | ----------- | ------------ | ------------ | ------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| Base status tracking                    | ✓           | ✓            | ✓            | ✓            | Covers `running` and `idle`; richer error, waiting, and background states depend on agent-specific hooks                        |
| Prompt text display                      | ✓           | ✓            | ✓            | ✓            | Saved from the agent's prompt-start event                                                                                        |
| Response text display (`▷ ...`)          | ✓           | ✓            | ✓            | ✓            | Populated from the agent's completed-turn payload                                                                                |
| Background shell state                   | ✓           | —            | —            | —            | Claude Bash tools can report `run_in_background`; the other bridges do not forward shell arguments                              |
| Waiting status + wait reason             | ✓           | —            | ✓            | ✓            | OpenCode and OMP map permission prompts to waiting notifications                                                                 |
| API failure reason display               | ✓           | —            | ✓            | —            | `StopFailure` is wired only for Claude and OpenCode                                                                             |
| Permission badge                         | ✓ (`plan` / `edit` / `auto` / `!`) | ✓ (`auto` / `!` only) | — | — | Codex badges are inferred from process arguments; OpenCode and OMP do not expose Claude-style permission modes                 |
| Git branch display                       | ✓           | ✓            | ✓            | ✓            | Uses the pane `cwd`; Claude updates dynamically via `CwdChanged`                                                                |
| Elapsed time                             | ✓           | ✓            | ✓            | ✓            | Since the last prompt                                                                                                            |
| Task progress                            | ✓           | —            | —            | —            | Requires task lifecycle events not forwarded by the other bridges                                                               |
| Task lifecycle notifications             | ✓           | ✓ (`Stop` only) | ✓          | ✓            | Completed-turn notifications fire for all four; other notification types vary                                                   |
| Sub-agent display                        | ✓           | —            | —            | —            | Requires `SubagentStart` / `SubagentStop`                                                                                        |
| Activity log                             | ✓           | ✓ (Bash only) | ✓            | ✓            | OMP reports tool starts without forwarding tool arguments or results                                                            |
| Worktree lifecycle tracking              | ✓           | —            | —            | —            | Requires `WorktreeCreate` / `WorktreeRemove`                                                                                     |
