---
title: Agent support overview
description: What the sidebar shows for Claude Code and Codex, side by side.
---

Both Claude Code and Codex work with the sidebar, but they expose different sets of hooks — so the sidebar's surface area is narrower for Codex.

## Feature support by agent

| Feature                                   | Claude Code | Codex        | Notes                                                                                                                           |
| ----------------------------------------- | ----------- | ------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| Status tracking (running / idle / error)  | ✓           | ✓            | Driven by `SessionStart` / `UserPromptSubmit` / `Stop`                                                                          |
| Prompt text display                       | ✓           | ✓            | Saved from `UserPromptSubmit`                                                                                                   |
| Response text display (`▷ ...`)           | ✓           | ✓            | Populated from the `Stop` payload                                                                                                |
| Waiting status + wait reason              | ✓           | —            | Needs Claude-only `Notification`, `PermissionDenied`, `TeammateIdle`                                                             |
| API failure reason display                | ✓           | —            | `StopFailure` is wired only for Claude                                                                                           |
| Permission badge                          | ✓ (`plan` / `edit` / `auto` / `!`) | ✓ (`auto` / `!` only) | Codex badges are inferred from process arguments                                                                                 |
| Git branch display                        | ✓           | ✓            | Uses the pane `cwd`; Claude updates dynamically via `CwdChanged`                                                                |
| Elapsed time                              | ✓           | ✓            | Since the last prompt                                                                                                            |
| Task progress                             | ✓           | —            | Requires `PostToolUse`; Codex fires `PostToolUse` only for `Bash`, so task progress from tools is unavailable                    |
| Task lifecycle notifications              | ✓           | ✓ (`Stop` only) | `Stop` desktop notifications fire for both. `Notification`, `TaskCompleted`, `StopFailure`, and `PermissionDenied` are Claude-only |
| Sub-agent display                         | ✓           | —            | Requires `SubagentStart` / `SubagentStop`                                                                                        |
| Activity log                              | ✓           | ✓ (Bash only) | Codex's `PostToolUse` fires only for `Bash` tool calls                                                                           |
| Worktree lifecycle tracking               | ✓           | —            | Requires `WorktreeCreate` / `WorktreeRemove`                                                                                     |
