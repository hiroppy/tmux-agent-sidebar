---
title: GitHub Copilot CLI setup
description: Wire up GitHub Copilot CLI's hooks to report into the sidebar.
---

GitHub Copilot CLI hooks use a different config shape from Claude/Codex/
Devin: command hook entries use `bash` / `powershell` fields instead of a
single `command` field, and the file has a `"version"` + `"hooks"`
top-level wrapper with camelCase event names.

## Config file

Copilot CLI reads hook config from (in load order — repository-level, then
user-level):

- `.github/hooks/*.json` — repository-level, typically committed.
- `~/.copilot/hooks/*.json` — user-level (`%USERPROFILE%\.copilot\hooks\`
  on Windows).
- An inline `"hooks"` key in `.github/copilot/settings.json` /
  `~/.copilot/settings.json`.

## Snippet

Create `.github/hooks/tmux-agent-sidebar.json`:

```json
{
  "version": 1,
  "hooks": {
    "sessionStart": [
      { "type": "command", "bash": "~/.tmux/plugins/tmux-agent-sidebar/hook.sh copilot session-start" }
    ],
    "sessionEnd": [
      { "type": "command", "bash": "~/.tmux/plugins/tmux-agent-sidebar/hook.sh copilot session-end" }
    ],
    "userPromptSubmitted": [
      { "type": "command", "bash": "~/.tmux/plugins/tmux-agent-sidebar/hook.sh copilot user-prompt-submit" }
    ],
    "agentStop": [
      { "type": "command", "bash": "~/.tmux/plugins/tmux-agent-sidebar/hook.sh copilot stop" }
    ],
    "postToolUse": [
      { "type": "command", "bash": "~/.tmux/plugins/tmux-agent-sidebar/hook.sh copilot activity-log" }
    ],
    "notification": [
      { "type": "command", "bash": "~/.tmux/plugins/tmux-agent-sidebar/hook.sh copilot notification" }
    ]
  }
}
```

The `notification` hook is what makes the sidebar show **waiting** (rather
than running) when Copilot needs something from you — both for a plain
permission prompt (`notification_type: "permission_prompt"`) and for a
multi-choice / clarifying question the agent asks
(`notification_type: "elicitation_dialog"`). Without it wired, the sidebar
has no way to tell "actively working" apart from "blocked on your answer".

Adjust the `hook.sh` path if your plugin lives somewhere other than
`~/.tmux/plugins/tmux-agent-sidebar`. On Windows, also set a `powershell`
entry alongside `bash` (Copilot CLI uses whichever matches the platform).

## Restart Copilot CLI

Restart Copilot CLI after adding or editing the hook config so it
discovers the new hooks.
