---
title: omp setup
description: Wire up the omp (oh-my-pi) extension bridge from the bundled extension directory.
---

omp uses the bundled extension bridge. Once you make the extension visible to
omp, the sidebar can receive its events automatically.

## Extension bridge

### Link the extension file

Create omp's global extensions directory if it does not already exist, then
symlink the extension **file** (not the directory) into it. Linking the single
file lets the bridge coexist with any other extensions you have installed:

```sh
mkdir -p ~/.omp/agent/extensions
ln -sf ~/.tmux/plugins/tmux-agent-sidebar/.omp/extensions/tmux-agent-sidebar.ts \
  ~/.omp/agent/extensions/tmux-agent-sidebar.ts
```

If you keep `tmux-agent-sidebar` in a different path, point the symlink at
that copy instead. The bridge resolves the symlink to find `hook.sh`, so it
works from either location.

The extension does nothing outside tmux: it exits at load time when
`$TMUX_PANE` is not set.

### Restart omp

Restart omp after adding the extension so it discovers the new bridge.
