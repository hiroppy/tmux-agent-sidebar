---
title: OpenCode setup
description: Wire up the OpenCode plugin bridge from the bundled plugin directory.
---

OpenCode uses the bundled plugin bridge. Once you make the plugin visible to
OpenCode, the sidebar can receive its events automatically.

## Plugin bridge

### Expose the bundled plugin directory

Create OpenCode's global plugin directory if it does not already exist, then
symlink the bundled plugin folder into it:

```sh
mkdir -p ~/.config/opencode/plugins
ln -s ~/.tmux/plugins/tmux-agent-sidebar/.opencode/plugins \
  ~/.config/opencode/plugins/tmux-agent-sidebar
```

If you keep `tmux-agent-sidebar` in a different path, point the symlink at that
copy instead.

### Restart OpenCode

Restart OpenCode after adding the plugin so it discovers the new bridge.
