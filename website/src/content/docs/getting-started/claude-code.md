---
title: Claude Code setup
description: Install the Claude Code plugin that ships with the sidebar.
---

The repository ships as a Claude Code plugin, so setup is automatic.

## Plugin

### Register the marketplace and install

Inside Claude Code:

```text
/plugin marketplace add ~/.tmux/plugins/tmux-agent-sidebar
/plugin install tmux-agent-sidebar@hiroppy
```

Either install path wires up the Claude Code hooks.

### Reload the plugin

Run `/reload-plugins` inside Claude Code (or restart it) to activate them.

## Manual setup

If your environment can't use the plugin, you can register hooks in `settings.json` directly. Paste this prompt into Claude Code:

```text
Run ~/.tmux/plugins/tmux-agent-sidebar/target/release/tmux-agent-sidebar setup claude
(fall back to ~/.tmux/plugins/tmux-agent-sidebar/bin/tmux-agent-sidebar if that path
is missing). Add these hooks to ~/.claude/settings.json. If hooks already exist,
merge them without making destructive changes.
```
