---
title: Agent support overview
description: What the sidebar shows for Claude Code, Codex, OpenCode, and Cursor, side by side.
---

Claude Code, Codex, OpenCode, and Cursor all work with the sidebar, but they expose different sets of hooks — so the sidebar's surface area is narrower for Codex, OpenCode, and Cursor than it is for Claude Code.

## Feature support by agent

| Feature                                  | Claude Code | Codex        | OpenCode     | Cursor       | Notes                                                                                                                           |
| ---------------------------------------- | ----------- | ------------ | ------------ | ------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| Base status tracking                    | ✓           | ✓            | ✓            | ✓            | Covers `running`, `idle`, and `error`; `waiting` and `background` depend on agent-specific hooks                                |
| Prompt text display                      | ✓           | ✓            | ✓            | —            | Saved from `UserPromptSubmit`; Cursor's `beforeSubmitPrompt` does not fire in the CLI                                           |
| Response text display (`▷ ...`)          | ✓           | ✓            | ✓            | —            | Populated from the `Stop` payload; Cursor's `stop` carries no message and `afterAgentResponse` does not fire in the CLI          |
| Background shell state                   | ✓           | —            | —            | —            | Claude Bash tools can report `run_in_background`; the others do not document a background Bash flag                             |
| Waiting status + wait reason             | ✓           | —            | ✓            | —            | OpenCode maps permission prompts to waiting notifications; Claude also has `Notification`, `PermissionDenied`, and `TeammateIdle` |
| API failure reason display               | ✓           | —            | ✓            | ✓ (generic)  | `StopFailure` for Claude/OpenCode; Cursor infers it from `stop` with `status: "error"`, which carries no message                 |
| Permission badge                         | ✓ (`plan` / `edit` / `auto` / `!`) | ✓ (`auto` / `!` only) | — | — | Codex badges are inferred from process arguments; OpenCode and Cursor do not expose permission modes                            |
| Git branch display                       | ✓           | ✓            | ✓            | ✓            | Uses the pane `cwd`; Claude updates dynamically via `CwdChanged`                                                                |
| Elapsed time                             | ✓           | ✓            | ✓            | ✓            | Since the last prompt                                                                                                            |
| Task progress                            | ✓           | —            | —            | —            | Requires `PostToolUse`; Codex fires `PostToolUse` only for `Bash`, and OpenCode/Cursor surface no task tool                     |
| Task lifecycle notifications             | ✓           | ✓ (`Stop` only) | ✓         | ✓ (`Stop` only) | `Stop` desktop notifications fire for all four. `Notification`, `TaskCompleted`, `StopFailure`, and `PermissionDenied` vary.  |
| Sub-agent display                        | ✓           | —            | —            | —            | Requires `SubagentStart` / `SubagentStop`                                                                                        |
| Activity log                             | ✓           | ✓ (Bash only) | ✓           | ✓            | Codex's `PostToolUse` fires only for `Bash`; OpenCode records what the plugin bridge receives; Cursor records `postToolUse`      |
| Worktree lifecycle tracking              | ✓           | —            | —            | —            | Requires `WorktreeCreate` / `WorktreeRemove`                                                                                     |
