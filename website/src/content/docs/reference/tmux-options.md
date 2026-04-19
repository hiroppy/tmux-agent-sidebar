---
title: tmux options
description: Every @sidebar_* / @agent-sidebar-* option the plugin reads.
---

Most options must be set **before** loading the plugin in your `tmux.conf`. Color codes are xterm 256-color numbers (0–255); icons can be any Unicode glyph.

## Sidebar behavior

| Option                           | Default | Description                                                                             |
| -------------------------------- | ------- | --------------------------------------------------------------------------------------- |
| `@sidebar_key`                   | `e`     | Prefix-triggered keybinding to toggle the sidebar in the current window                 |
| `@sidebar_key_all`               | `E`     | Prefix-triggered keybinding to toggle the sidebar in all windows                        |
| `@sidebar_width`                 | `15%`   | Width in columns or as a percentage                                                     |
| `@sidebar_bottom_height`         | `20`    | Bottom panel height in lines (set `0` to hide)                                          |
| `@sidebar_auto_create`           | `on`    | Auto-create the sidebar on new windows (set `off` to disable)                           |
| `@sidebar_notifications`         | `on`    | Master switch for desktop notifications                                                 |
| `@sidebar_notifications_events`  | unset   | Restrict events — see [Notifications](/tmux-agent-sidebar/features/notifications/)       |

## Worktree spawn defaults

| Option                            | Default  | Description                                  |
| --------------------------------- | -------- | -------------------------------------------- |
| `@agent-sidebar-default-agent`    | `claude` | Agent launched by `n`&nbsp;(also accepts `codex`) |
| `@agent-sidebar-branch-prefix`    | `agent/` | Branch prefix for new worktrees              |

## Status and filter colors

| Option                            | Default         | What it paints                                                    |
| --------------------------------- | --------------- | ----------------------------------------------------------------- |
| `@sidebar_color_all`              | `111`&nbsp;(sky blue)| Selected "all" filter icon                                        |
| `@sidebar_color_running`          | `114`&nbsp;(green)   | Selected running filter icon and running pane status              |
| `@sidebar_color_waiting`          | `221`&nbsp;(yellow)  | Selected waiting filter icon, waiting pane status, version banner |
| `@sidebar_color_idle`             | `110`&nbsp;(soft blue) | Selected idle filter icon and idle pane status                  |
| `@sidebar_color_error`            | `167`&nbsp;(soft red) | Selected error filter icon and error pane status                 |
| `@sidebar_color_filter_inactive`  | `245`&nbsp;(mid gray) | Unselected status filter icons and zero counts                   |

## Structural colors

| Option                     | Default              | What it paints                                                                                          |
| -------------------------- | -------------------- | ------------------------------------------------------------------------------------------------------- |
| `@sidebar_color_border`    | `240`&nbsp;(dark gray)    | Unfocused panel borders and tab separators                                                              |
| `@sidebar_color_accent`    | `153`&nbsp;(pale sky blue) | Active pane marker, focused repo header, focused bottom panel border, repo popup border — the brand color |
| `@sidebar_color_session`   | `39`&nbsp;(blue)          | Session name                                                                                            |
| `@sidebar_color_selection` | `239`&nbsp;(dark gray)    | Selected row background                                                                                 |

## Agent colors

| Option                          | Default            | What it paints       |
| ------------------------------- | ------------------ | -------------------- |
| `@sidebar_color_agent_claude`   | `174`&nbsp;(terracotta) | Claude brand color   |
| `@sidebar_color_agent_codex`    | `141`&nbsp;(purple)     | Codex brand color    |

## Text colors

| Option                         | Default          | What it paints                                                                                   |
| ------------------------------ | ---------------- | ------------------------------------------------------------------------------------------------ |
| `@sidebar_color_text_active`   | `255`&nbsp;(white)    | Primary text — active rows, counts, filtered repo label                                          |
| `@sidebar_color_text_muted`    | `252`&nbsp;(light gray) | Secondary text — tree branches, empty-state messages, inactive bottom tabs, activity log labels |
| `@sidebar_color_text_inactive` | `244`&nbsp;(mid gray) | Body text of unfocused pane rows — prompt / response, idle hint                                  |
| `@sidebar_color_port`          | `246`&nbsp;(light gray) | Port numbers                                                                                   |
| `@sidebar_color_wait_reason`   | `221`&nbsp;(yellow)   | Wait reason text                                                                                 |
| `@sidebar_color_response_arrow`| `81`&nbsp;(bright cyan) | Response arrow                                                                                 |

## Task and sub-agent colors

| Option                          | Default           | What it paints        |
| ------------------------------- | ----------------- | --------------------- |
| `@sidebar_color_task_progress`  | `223`&nbsp;(pale yellow) | Task progress summary |
| `@sidebar_color_subagent`       | `73`&nbsp;(soft teal)  | Sub-agent tree        |

## Git tab colors

| Option                          | Default            | What it paints      |
| ------------------------------- | ------------------ | ------------------- |
| `@sidebar_color_branch`         | `109`&nbsp;(teal)       | Git branch name     |
| `@sidebar_color_commit_hash`    | `221`&nbsp;(yellow)     | Commit hash         |
| `@sidebar_color_diff_added`     | `114`&nbsp;(green)      | Added diff lines    |
| `@sidebar_color_diff_deleted`   | `174`&nbsp;(terracotta) | Deleted diff lines  |
| `@sidebar_color_file_change`    | `221`&nbsp;(yellow)     | File change stats   |
| `@sidebar_color_pr_link`        | `117`&nbsp;(light blue) | PR link / number    |

## Section titles and timestamps

| Option                                | Default      | What it paints      |
| ------------------------------------- | ------------ | ------------------- |
| `@sidebar_color_section_title`        | `109`&nbsp;(teal) | Section titles      |
| `@sidebar_color_activity_timestamp`   | `109`&nbsp;(teal) | Activity timestamps |

## Status icons

Any Unicode glyph works. Make sure the glyphs render in your terminal font.

| Option                  | Default | Meaning                      |
| ----------------------- | ------- | ---------------------------- |
| `@sidebar_icon_all`     | `≡`     | Status filter bar "all" icon |
| `@sidebar_icon_running` | `●`     | Running status icon          |
| `@sidebar_icon_waiting` | `◐`     | Waiting status icon          |
| `@sidebar_icon_idle`    | `○`     | Idle status icon             |
| `@sidebar_icon_error`   | `✕`     | Error status icon            |
| `@sidebar_icon_unknown` | `·`     | Unknown status icon          |

## Example config

```bash
# Behavior
set -g @sidebar_key T
set -g @sidebar_width 32
set -g @sidebar_bottom_height 25
set -g @sidebar_notifications_events "stop,notification"
set -g @agent-sidebar-default-agent codex

# Colors
set -g @sidebar_color_accent 117
set -g @sidebar_color_agent_claude 203

# Icons
set -g @sidebar_icon_running '▶'
set -g @sidebar_icon_error   '⚠'

run-shell ~/.tmux/plugins/tmux-agent-sidebar/tmux-agent-sidebar.tmux
```
