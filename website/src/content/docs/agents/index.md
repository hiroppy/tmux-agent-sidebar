---
title: Agent support overview
description: What the sidebar shows for Claude Code, Codex, Grok Build, and OpenCode, side by side.
---

Claude Code, Codex, Grok Build, and OpenCode all work with the sidebar. Their upstream event surfaces differ, so some features remain agent-specific.

## Feature support by agent

| Feature | Claude Code | Codex | Grok Build | OpenCode | Notes |
| ------- | ----------- | ----- | ---------- | -------- | ----- |
| Base status tracking | ✓ | ✓ | ✓ | ✓ | Covers `running`, `idle`, and `error`; other states depend on agent-specific hooks |
| Prompt text display | ✓ | ✓ | ✓ | ✓ | Saved from the agent's prompt event |
| Response text display (`▷ ...`) | ✓ | ✓ | ✓ | ✓ | Populated from the turn-completion payload |
| Background shell state | ✓ | — | ✓ | — | Grok marks native terminal calls with `background: true` |
| Waiting status + wait reason | ✓ | — | ✓ | ✓ | Grok uses `Notification:permission_prompt` |
| API failure reason display | ✓ | — | ✓ | ✓ | Grok uses `StopFailure` |
| Permission badge | ✓ | ✓ (`auto` / `!`) | ✓ | — | Grok reports `permissionMode` directly |
| Git branch display | ✓ | ✓ | ✓ | ✓ | Uses the pane `cwd` |
| Elapsed time | ✓ | ✓ | ✓ | ✓ | Since the last prompt |
| Task progress | ✓ | — | — | — | Requires the sidebar's task-list lifecycle |
| Task lifecycle notifications | ✓ | ✓ (`Stop` only) | ✓ | ✓ | Hook coverage varies by agent |
| Sub-agent display | ✓ | — | ✓ | — | Grok uses `SubagentStart` / `SubagentStop` |
| Activity log | ✓ | ✓ (Bash only) | ✓ | ✓ | Grok native tool names are normalized into sidebar categories |
| Worktree lifecycle tracking | ✓ | — | — | — | Sidebar-created worktrees remain available for every agent |
