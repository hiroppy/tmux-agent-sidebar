---
title: Agent support overview
description: What the sidebar shows for Claude Code, Codex, OpenCode, and omp, side by side.
---

Claude Code, Codex, OpenCode, and omp work with the sidebar, but they expose different sets of hooks — so the sidebar's surface area is narrower for Codex, OpenCode, and omp than it is for Claude Code.

## Feature support by agent

| Feature                                  | Claude Code | Codex        | OpenCode     | omp          | Notes                                                                                                                           |
| ---------------------------------------- | ----------- | ------------ | ------------ | ------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| Base status tracking                    | ✓           | ✓            | ✓            | ✓            | Covers `running`, `idle`, and `error`; `waiting` and `background` depend on agent-specific hooks                                |
| Prompt text display                      | ✓           | ✓            | ✓            | ✓            | Saved from `UserPromptSubmit` (omp: `before_agent_start`)                                                                       |
| Response text display (`▷ ...`)          | ✓           | ✓            | ✓            | ✓            | Populated from the `Stop` payload (omp: debounced `agent_end`)                                                                  |
| Background shell state                   | ✓           | —            | —            | —            | Claude Bash tools can report `run_in_background`; Codex, OpenCode, and omp do not currently document a background Bash flag       |
| Waiting status + wait reason             | ✓           | —            | ✓            | ✓            | OpenCode maps permission prompts to waiting notifications; omp maps tool-approval requests and `ask` dialogs; Claude also has `Notification`, `PermissionDenied`, and `TeammateIdle` |
| API failure reason display               | ✓           | —            | ✓            | ✓            | `StopFailure` is wired for Claude, OpenCode, and omp                                                                            |
| Permission badge                         | ✓ (`plan` / `edit` / `auto` / `!`) | ✓ (`auto` / `!` only) | — | — | Codex badges are inferred from process arguments; OpenCode and omp do not expose permission modes                               |
| Git branch display                       | ✓           | ✓            | ✓            | ✓            | Uses the pane `cwd`; Claude updates dynamically via `CwdChanged`                                                                |
| Elapsed time                             | ✓           | ✓            | ✓            | ✓            | Since the last prompt                                                                                                            |
| Task progress                            | ✓           | —            | —            | —            | Requires `PostToolUse`; Codex fires `PostToolUse` only for `Bash`, and OpenCode/omp do not surface task progress                |
| Task lifecycle notifications             | ✓           | ✓ (`Stop` only) | ✓            | ✓            | `Stop` desktop notifications fire for all four. `Notification`, `TaskCompleted`, `StopFailure`, and `PermissionDenied` vary.   |
| Sub-agent display                        | ✓           | —            | —            | —            | Requires `SubagentStart` / `SubagentStop`; omp subagent sessions are ignored so they never overwrite the parent pane's state      |
| Activity log                             | ✓           | ✓ (Bash only) | ✓            | ✓            | Codex's `PostToolUse` fires only for `Bash`; OpenCode and omp record the tool events their bridge receives (omp normalizes its built-in tools; other tools log unlabeled) |
| Worktree lifecycle tracking              | ✓           | —            | —            | —            | Requires `WorktreeCreate` / `WorktreeRemove`                                                                                     |
