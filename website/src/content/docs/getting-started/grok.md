---
title: Grok Build setup
description: Install tmux-agent-sidebar hooks from inside a Grok Build pane.
---

Grok Build loads JSON hook files from `~/.grok/hooks/`. The sidebar uses a dedicated file so it can coexist with your other Grok hooks.

## Steps

1. Open a Grok Build pane in tmux and focus it.
2. Press `prefix + e` to toggle the sidebar. A yellow `ⓘ` badge appears when required hooks are missing.
3. Click `ⓘ`, then click `[copy]` next to `grok` in the Notices popup.
4. Switch back to the Grok pane and paste. Grok runs `tmux-agent-sidebar setup grok` and writes or non-destructively merges the generated config into `~/.grok/hooks/tmux-agent-sidebar.json`.
5. Open Grok's `/hooks` view to reload the file, or restart Grok.

The generated config registers lifecycle, permission, subagent, and tool hooks. Its `Stop` handler is observational: it writes no decision output and exits successfully, so it cannot block Grok from stopping.

See the [Grok Build hooks documentation](https://docs.x.ai/build/features/hooks) for the upstream hook-file and reload behavior.
