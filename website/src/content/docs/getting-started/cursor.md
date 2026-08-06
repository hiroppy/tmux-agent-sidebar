---
title: Cursor setup
description: Wire up the Cursor CLI hooks from inside a Cursor pane.
---

Cursor CLI registers hooks through the same one-paste flow as Codex, driven by
the sidebar itself.

## Steps

1. Open a Cursor CLI pane in tmux (`agent`, or `cursor-agent` on older installations) and focus it.
2. Press `prefix + e` to toggle the sidebar. A yellow `ⓘ` badge appears in the top row when required hooks are missing.
3. Click `ⓘ`, then click `[copy]` next to `cursor` in the Notices popup.
4. Switch back to the Cursor pane and paste. Cursor runs `tmux-agent-sidebar setup cursor` and merges the hooks into `~/.cursor/hooks.json`.
5. Restart the Cursor CLI so it re-reads the hook config.

## Manual wiring

If you would rather edit the file yourself, run:

```sh
~/.tmux/plugins/tmux-agent-sidebar/target/release/tmux-agent-sidebar setup cursor
```

If you installed a pre-built release, use
`~/.tmux/plugins/tmux-agent-sidebar/bin/tmux-agent-sidebar` in place of the
`target/release` path above.

and merge the printed block into `~/.cursor/hooks.json`. Keep any `version`
value already in the file, and append to the per-trigger arrays rather than
replacing them so your existing hooks survive.

Project-level `.cursor/hooks.json` files work too, but the sidebar's
missing-hooks check only inspects the user-level file at `~/.cursor/hooks.json`.

## What the sidebar can see

Cursor documents far more hooks than its CLI currently fires, so only four are
wired: `sessionStart`, `sessionEnd`, `stop`, and `postToolUse`. See
[Cursor](/tmux-agent-sidebar/agents/cursor/) for the resulting feature coverage
and the gaps that come with it.
