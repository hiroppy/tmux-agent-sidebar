---
title: Pi setup
description: Wire up the Pi extension bridge from the bundled extension directory.
---

Pi uses the bundled TypeScript extension bridge. Once you make the extension
visible to Pi, the sidebar receives Pi's session, agent, and tool events
automatically.

## Extension bridge

### Link the extension file

Create Pi's extensions directory if it does not already exist, then symlink
the extension **file** (not the directory) into it. Linking the single file
lets the bridge coexist with any other extensions you have installed:

```sh
mkdir -p ~/.pi/agent/extensions
ln -sf ~/.tmux/plugins/tmux-agent-sidebar/.pi/extensions/tmux-agent-sidebar.ts \
  ~/.pi/agent/extensions/tmux-agent-sidebar.ts
```

If you keep `tmux-agent-sidebar` in a different path, point the symlink at
that copy instead. Pi tracks the symlink, and the bridge locates `hook.sh` by
walking up from its real path, so the target directory can live anywhere.

### Restart Pi

Restart Pi after adding the extension, or run `/reload`, so it discovers the
new bridge.

### Alternative: reference the path in settings

Instead of symlinking you can list the extension by absolute path in
`~/.pi/agent/settings.json`:

```json
{
  "extensions": [
    "/path/to/tmux-agent-sidebar/.pi/extensions/tmux-agent-sidebar.ts"
  ]
}
```

Keep any existing entries in `extensions`.
