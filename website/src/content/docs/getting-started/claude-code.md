---
title: Claude Code setup
description: Install the Claude Code plugin that ships with the sidebar.
---

The repository ships as a Claude Code plugin. Once you install and reload the plugin (steps below), the hooks wire up automatically.

## Plugin

### Register the marketplace and install

Inside Claude Code:

```text
/plugin marketplace add ~/.tmux/plugins/tmux-agent-sidebar
/plugin install tmux-agent-sidebar@hiroppy
```

The `/plugin install` step wires up the Claude Code hooks.

### Reload the plugin

Run `/reload-plugins` inside Claude Code (or restart it) to activate them.

## Manual setup

If your environment can't use the plugin, you can register hooks in `settings.json` directly. Paste this prompt into Claude Code:

```text
Run ~/.tmux/plugins/tmux-agent-sidebar/target/release/tmux-agent-sidebar setup claude
(fall back to ~/.tmux/plugins/tmux-agent-sidebar/bin/tmux-agent-sidebar if that
path is missing). From the command output, take the `hooks` object and merge it
into the `hooks` key of ~/.claude/settings.json — create the key if it does not
exist and preserve any entries already there.
```
