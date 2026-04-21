---
title: Codex setup
description: Wire up the Codex hooks from inside a Codex pane.
---

Codex registers hooks through a one-paste flow driven by the sidebar itself.

Before Codex will load hooks, enable the feature flag in `~/.codex/config.toml`:

```toml
[features]
codex_hooks = true
```

If you already have a `[features]` table, add `codex_hooks = true` under it.

## Steps

1. Open a Codex pane in tmux and focus it.
2. Press `prefix + e` to toggle the sidebar. A yellow `ⓘ` badge appears in the top row when required hooks are missing.
3. Click `ⓘ`, then click `[copy]` next to `codex` in the Notices popup.
4. Switch back to the Codex pane and paste. Codex runs `tmux-agent-sidebar setup codex` and merges the hooks into `~/.codex/hooks.json`.
5. Restart Codex after changing `config.toml` so the feature flag takes effect.
