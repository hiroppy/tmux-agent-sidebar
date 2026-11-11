---
title: Agent support overview
description: What the sidebar shows for Claude Code, Codex, OpenCode, and pi, side by side.
---

Claude Code, Codex, OpenCode, and pi work with the sidebar, but they expose different sets of hooks — so the sidebar's surface area is narrower for Codex, OpenCode, and pi than it is for Claude Code.

## Feature support by agent

| Feature                                  | Claude Code | Codex        | OpenCode     | pi           | Notes                                                                                                                           |
| ----------------------------------------- | ----------- | ------------ | ------------ | ------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| Base status tracking                    | ✓           | ✓            | ✓            | ✓            | Covers `running`, `idle`, and `error`; `waiting` and `background` depend on agent-specific hooks                                |
| Prompt text display                      | ✓           | ✓            | ✓            | ✓            | Saved from `UserPromptSubmit` (pi: `before_agent_start`)                                                                        |
| Response text display (`▷ ...`)          | ✓           | ✓            | ✓            | ✓            | Populated from the `Stop` payload (pi: `agent_settled`)                                                                         |
| Background shell state                   | ✓           | —            | —            | —            | Claude Bash tools can report `run_in_background`; Codex, OpenCode, and pi do not currently document a background Bash flag        |
| Waiting status + wait reason             | ✓           | —            | ✓            | —            | OpenCode maps permission prompts to waiting notifications; Claude also has `Notification`, `PermissionDenied`, and `TeammateIdle`; pi exposes no permission-approval event to extensions |
| API failure reason display               | ✓           | —            | ✓            | —            | `StopFailure` is wired only for Claude and OpenCode; pi has no dedicated "agent errored" event                                  |
| Permission badge                         | ✓ (`plan` / `edit` / `auto` / `!`) | ✓ (`auto` / `!` only) | — | — | Codex badges are inferred from process arguments; OpenCode and pi do not expose permission modes                                |
| Git branch display                       | ✓           | ✓            | ✓            | ✓            | Uses the pane `cwd`; Claude updates dynamically via `CwdChanged`                                                                |
| Elapsed time                             | ✓           | ✓            | ✓            | ✓            | Since the last prompt                                                                                                            |
| Task progress                            | ✓           | —            | —            | —            | Requires `PostToolUse`; Codex fires `PostToolUse` only for `Bash`, and OpenCode/pi do not surface task progress                 |
| Task lifecycle notifications             | ✓           | ✓ (`Stop` only) | ✓            | ✓ (`Stop` only) | `Stop` desktop notifications fire for all four. `Notification`, `TaskCompleted`, `StopFailure`, and `PermissionDenied` vary.  |
| Sub-agent display                        | ✓           | —            | —            | —            | Requires `SubagentStart` / `SubagentStop`                                                                                        |
| Activity log                             | ✓           | ✓ (Bash only) | ✓            | ✓            | Codex's `PostToolUse` fires only for `Bash`; OpenCode and pi record the tool events their bridge receives (pi normalizes `bash`/`read`/`write`/`edit`/`powershell`; other tools log unlabeled) |
| Worktree lifecycle tracking              | ✓           | —            | —            | —            | Requires `WorktreeCreate` / `WorktreeRemove`                                                                                     |
