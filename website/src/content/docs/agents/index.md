---
title: Agent support overview
description: What the sidebar shows for Claude Code, Codex, OpenCode, and Pi, side by side.
---

Claude Code, Codex, OpenCode, and Pi work with the sidebar, but they expose different sets of hooks — so the sidebar's surface area is narrower for Codex, OpenCode, and Pi than it is for Claude Code.

## Feature support by agent

| Feature                                  | Claude Code | Codex        | OpenCode     | Pi           | Notes                                                                                                                           |
| ---------------------------------------- | ----------- | ------------ | ------------ | ------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| Base status tracking                    | ✓           | ✓            | ✓            | ✓            | Covers `running`, `idle`, and `error`; `waiting` and `background` depend on agent-specific hooks                                |
| Prompt text display                      | ✓           | ✓            | ✓            | ✓            | Saved from `UserPromptSubmit`                                                                                                   |
| Response text display (`▷ ...`)          | ✓           | ✓            | ✓            | —            | Populated from the `Stop` payload; the Pi bridge forwards `Stop` without final assistant text                                  |
| Background shell state                   | ✓           | —            | —            | —            | Claude Bash tools can report `run_in_background`; Codex, OpenCode, and Pi do not currently document a background Bash flag        |
| Waiting status + wait reason             | ✓           | —            | ✓            | ✓            | OpenCode maps permission prompts, Pi maps blocking UI prompts; Claude also has `Notification`, `PermissionDenied`, and `TeammateIdle` |
| API failure reason display               | ✓           | —            | ✓            | ✓            | `StopFailure` is wired for Claude, OpenCode, and Pi                                                                             |
| Permission badge                         | ✓ (`plan` / `edit` / `auto` / `!`) | ✓ (`auto` / `!` only) | — | — | Codex badges are inferred from process arguments; OpenCode and Pi do not expose permission modes                                |
| Git branch display                       | ✓           | ✓            | ✓            | ✓            | Uses the pane `cwd`; Claude updates dynamically via `CwdChanged`                                                                |
| Elapsed time                             | ✓           | ✓            | ✓            | ✓            | Since the last prompt                                                                                                            |
| Task progress                            | ✓           | —            | —            | —            | Requires `PostToolUse`; Codex fires `PostToolUse` only for `Bash`, and OpenCode and Pi do not surface task progress              |
| Task lifecycle notifications             | ✓           | ✓ (`Stop` only) | ✓         | ✓ (`Stop` only) | `Stop` desktop notifications fire for all four. `Notification`, `TaskCompleted`, `StopFailure`, and `PermissionDenied` vary.     |
| Sub-agent display                        | ✓           | —            | —            | —            | Requires `SubagentStart` / `SubagentStop`                                                                                        |
| Activity log                             | ✓           | ✓ (Bash only) | ✓            | ✓            | Codex's `PostToolUse` fires only for `Bash`; OpenCode and Pi record the tool events their bridges receive                       |
| Worktree lifecycle tracking              | ✓           | —            | —            | —            | Requires `WorktreeCreate` / `WorktreeRemove`                                                                                     |
