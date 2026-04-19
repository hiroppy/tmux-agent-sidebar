---
title: Installation
description: Install tmux-agent-sidebar via TPM or manually.
---

## Requirements

- tmux 3.0+
- [TPM](https://github.com/tmux-plugins/tpm) (for plugin installation)
- [GitHub CLI](https://cli.github.com/) (optional, for displaying PR numbers in the Git tab)
- [Rust](https://rustup.rs/) (only if building from source)

## Option A — TPM (recommended)

Add the plugin to your `tmux.conf`:

```bash
set -g @plugin 'hiroppy/tmux-agent-sidebar'
```

Reload `tmux.conf`, then press `prefix + I` to install:

```sh
tmux source ~/.tmux.conf
```

On the first run, an install wizard prompts you to download a pre-built binary or build from source.

To update later, press `prefix + U` in TPM's plugin list and select `tmux-agent-sidebar`. The install wizard runs again if the bundled binary has changed.

## Option B — Manual

1. Clone the repository:

   ```sh
   git clone https://github.com/hiroppy/tmux-agent-sidebar.git \
     ~/.tmux/plugins/tmux-agent-sidebar
   ```

2. Add the plugin to your `tmux.conf`:

   ```bash
   run-shell ~/.tmux/plugins/tmux-agent-sidebar/tmux-agent-sidebar.tmux
   ```

3. Install the binary — download a pre-built release, or build from source:

   ```sh
   # macOS (Apple Silicon)
   curl -fSL https://github.com/hiroppy/tmux-agent-sidebar/releases/latest/download/tmux-agent-sidebar-darwin-aarch64 \
     -o ~/.tmux/plugins/tmux-agent-sidebar/bin/tmux-agent-sidebar
   chmod +x ~/.tmux/plugins/tmux-agent-sidebar/bin/tmux-agent-sidebar
   ```

   Or build from source:

   ```sh
   cd ~/.tmux/plugins/tmux-agent-sidebar
   cargo build --release
   ```

## Reload tmux config

After editing `tmux.conf`, press `prefix + r` (or run `tmux source ~/.tmux.conf`) to reload.

## Next steps

The sidebar receives status updates through agent hooks — continue with the agent you use:

- [Claude Code setup](/tmux-agent-sidebar/getting-started/claude-code/)
- [Codex setup](/tmux-agent-sidebar/getting-started/codex/)
