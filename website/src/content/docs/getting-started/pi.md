---
title: pi setup
description: Wire up the pi extension bridge from the bundled extension directory.
---

pi uses the bundled extension bridge. Once you make the extension visible to
pi, the sidebar can receive its events automatically.

## Extension bridge

### Link the extension file

Create pi's global extensions directory if it does not already exist, then
symlink the extension **file** (not the directory) into it. Linking the
single file lets the bridge coexist with any other extensions you have
installed, and installing into the global directory means it loads for
every project without needing a per-project trust decision:

```sh
mkdir -p ~/.pi/agent/extensions
ln -sf ~/.tmux/plugins/tmux-agent-sidebar/.pi/extensions/tmux-agent-sidebar.ts \
  ~/.pi/agent/extensions/tmux-agent-sidebar.ts
```

If you keep `tmux-agent-sidebar` in a different path, point the symlink at
that copy instead.

> Prefer a project-local install instead? pi also auto-discovers
> `.pi/extensions/*.ts` inside a trusted project, but the extension will not
> load until that project has been trusted — the global install above avoids
> that gate entirely.

### Restart pi

Restart pi after adding the extension so it discovers the new bridge. You can
also run `/reload` inside an existing pi session instead of restarting.
