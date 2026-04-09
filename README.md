# tmux-agent-sidebar

A tmux sidebar that monitors all AI coding agents (Claude Code, Codex) across every session and window — statuses, prompts, git info, activity logs, and more in one place.

![main](assets/main.png)

<table>
  <tr>
    <td><img src="assets/category.png" alt="category view" /></td>
    <td><img src="assets/filter-by-project.png" alt="filter by project" /></td>
  </tr>
  <tr>
    <td align="center"><b>Category view</b></td>
    <td align="center"><b>Filter by project</b></td>
  </tr>
</table>

## Features

- **Cross-session monitoring** — Shows all agents across every tmux session and window in one sidebar
- **Pane jump** — Select an agent and press Enter to jump to its pane, even across different windows
- **Repository grouping** — Groups agents by the same repo, including worktrees, so related panes stay together
- **Status filter** — Filters by Running / Waiting / Idle / Error with live counts per status
- **Subagent tree** — Shows spawned subagents as a parent-child tree
- **Task progress** — Displays task completion (e.g. `3/7`) synced from agent task lists
- **Activity log** — Streams each tool invocation (Read, Edit, Bash, etc.) per agent in real time
- **Git integration** — Shows branch name, ahead/behind counts, PR number (`gh`), and per-file diff stats
- **Pane metadata** — Shows listening localhost ports and execution command info for each pane, alongside branch and elapsed time
- **Permission mode** — Shows `auto` / `plan` / `!` badge so you know each agent's permission level

## Requirements

- tmux 3.0+
- [TPM](https://github.com/tmux-plugins/tpm) (recommended, for plugin installation)
- [Rust](https://rustup.rs/) (only if building from source)
- [GitHub CLI](https://cli.github.com/) (optional, for PR number display in Git tab)

## Setting Up

### 1. Installation

#### Installation with TPM

Add the plugin to your `tmux.conf`:

```tmux
set -g @plugin 'hiroppy/tmux-agent-sidebar'
run '~/.tmux/plugins/tpm/tpm'
```

Press `prefix + I` to install. On first run, an install wizard will prompt you to download a pre-built binary or build from source.

#### Manual

1. Clone the repository:

```sh
git clone https://github.com/hiroppy/tmux-agent-sidebar.git ~/.tmux/plugins/tmux-agent-sidebar
```

1. Add the plugin to your `tmux.conf`:

```tmux
run-shell ~/.tmux/plugins/tmux-agent-sidebar/tmux-agent-sidebar.tmux
```

1. Install the binary using one of the following methods:

   <details>
   <summary>Download pre-built binary</summary>

   ```sh
   # macOS (Apple Silicon)
   curl -fSL https://github.com/hiroppy/tmux-agent-sidebar/releases/latest/download/tmux-agent-sidebar-darwin-aarch64 \
     -o ~/.tmux/plugins/tmux-agent-sidebar/bin/tmux-agent-sidebar
   chmod +x ~/.tmux/plugins/tmux-agent-sidebar/bin/tmux-agent-sidebar
   ```

   </details>

   <details>
   <summary>Build from source (requires Rust)</summary>

   ```sh
   cd ~/.tmux/plugins/tmux-agent-sidebar
   cargo build --release
   ```

   </details>

Reload tmux config with `prefix + r` after updating `tmux.conf`. This works for both TPM and manual installs.

### 2. Agent Hooks

The sidebar receives status updates through agent hooks. Add the following hook configurations to your agent settings.

#### 2.1 Claude Code

Add to your Claude Code hooks configuration (e.g. `~/.claude/settings.json`):

<details>
<summary>Claude Code hooks JSON</summary>

```json
{
  "hooks": {
    "Notification": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "bash ~/.tmux/plugins/tmux-agent-sidebar/hook.sh claude notification"
          }
        ]
      }
    ],
    "Stop": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "bash ~/.tmux/plugins/tmux-agent-sidebar/hook.sh claude stop"
          }
        ]
      }
    ],
    "StopFailure": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "bash ~/.tmux/plugins/tmux-agent-sidebar/hook.sh claude stop-failure"
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "bash ~/.tmux/plugins/tmux-agent-sidebar/hook.sh claude user-prompt-submit"
          }
        ]
      }
    ],
    "SessionStart": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "bash ~/.tmux/plugins/tmux-agent-sidebar/hook.sh claude session-start"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "bash ~/.tmux/plugins/tmux-agent-sidebar/hook.sh claude activity-log"
          }
        ]
      }
    ],
    "SessionEnd": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "bash ~/.tmux/plugins/tmux-agent-sidebar/hook.sh claude session-end"
          }
        ]
      }
    ]
  }
}
```

</details>

#### 2.2 Codex

Create or edit `~/.codex/hooks.json`:

<details>
<summary>Codex hooks JSON</summary>

```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "startup|resume",
        "hooks": [
          {
            "type": "command",
            "command": "bash ~/.tmux/plugins/tmux-agent-sidebar/hook.sh codex session-start"
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "bash ~/.tmux/plugins/tmux-agent-sidebar/hook.sh codex user-prompt-submit"
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "bash ~/.tmux/plugins/tmux-agent-sidebar/hook.sh codex stop"
          }
        ]
      }
    ],
    "SessionEnd": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "bash ~/.tmux/plugins/tmux-agent-sidebar/hook.sh codex session-end"
          }
        ]
      }
    ]
  }
}
```

</details>

## Usage

### Keybindings

| Key | Action |
|---|---|
| `prefix + e` | Toggle sidebar (default, customizable) |
| `prefix + E` | Toggle sidebar in all windows (default, customizable) |
| `j` / `Down` | Move selection down (filter → agents → bottom panel) |
| `k` / `Up` | Move selection up |
| `h` / `Left` | Previous filter (when on filter bar) |
| `l` / `Right` | Next filter (when on filter bar) |
| `Enter` | Jump to selected agent's pane |
| `Tab` | Cycle status filter (All → Running → Waiting → Idle → Error) |
| `Shift+Tab` | Switch bottom panel tab (Activity / Git) |
| `Esc` | Return focus to agents panel |
| Mouse click | Click agent to jump to its pane, click filter to select |

### Status Icons

| Icon | State | Description |
|---|---|---|
| `●` | Running | Agent is actively processing (pulsing animation) |
| `◐` | Waiting | Agent needs user attention (e.g. permission approval) |
| `○` | Idle | Agent is ready and waiting for input |
| `✕` | Error | Agent encountered an error |

### Feature Support by Agent

| Feature | Claude Code | Codex | Notes |
|---|---|---|---|
| Status tracking (running / idle / error) | :white_check_mark: | :white_check_mark: | Driven by `SessionStart` / `UserPromptSubmit` / `Stop` |
| Prompt text display | :white_check_mark: | :white_check_mark: | Saved from `UserPromptSubmit` |
| Response text display (`▶ ...`) | :white_check_mark: | :white_check_mark: | Populated from `Stop` payload |
| Waiting status + wait reason | :white_check_mark: | :x: | Codex has no `Notification` hook |
| API failure reason display | :white_check_mark: | :x: | `StopFailure` is wired only for Claude |
| Permission badge | :white_check_mark: (`plan` / `edit` / `auto` / `!`) | :white_check_mark: (`auto` / `!` only) | Codex badges are inferred from process args |
| Git branch display | :white_check_mark: | :white_check_mark: | Uses the pane `cwd` |
| Elapsed time | :white_check_mark: | :white_check_mark: | Since the last prompt |
| Task progress | :white_check_mark: | :x: | Requires `PostToolUse` |
| Subagent display | :white_check_mark: | :x: | Requires `SubagentStart` / `SubagentStop` |
| Activity log | :white_check_mark: | :x: | Requires `PostToolUse` |

### Known Limitations

- **Waiting status (Claude Code)** — After approving a permission prompt, the status stays `waiting` until the next hook event fires. This is a limitation of the Claude Code hook system.
- **Codex hook coverage** — Codex does not support `Notification` or `PostToolUse` hooks, so waiting status, activity log, and task progress are unavailable.
- **Dead pane cleanup** — If an agent exits without a hook, the periodic pid scan removes the stale pane on the next refresh cycle.

## Accessing Agent Status from Scripts

The sidebar stores agent status in tmux pane options, which you can read from your own scripts or status bar:

```sh
# Get a specific pane's agent status
tmux show -t "$pane_id" -pv @pane_status
# Returns: running / waiting / idle / error / (empty)

# Get agent type
tmux show -t "$pane_id" -pv @pane_agent
# Returns: claude / codex / (empty)
```

This is useful for integrating agent status into your tmux status bar, custom scripts, or notifications.

## Customization

All options can be set **before** loading the plugin in your `tmux.conf`:

```tmux
# Sidebar
set -g @sidebar_key T                    # keybinding (default: e)
set -g @sidebar_key_all Y                # keybinding for all windows (default: E)
set -g @sidebar_width 32                 # width in columns or % (default: 15%)

# Colors (256-color palette numbers)
set -g @sidebar_color_running 82         # running icon (default: green)
set -g @sidebar_color_waiting 221        # waiting icon (default: yellow)
set -g @sidebar_color_idle 250           # idle icon (default: light gray)
set -g @sidebar_color_error 203         # error icon (default: red)
set -g @sidebar_color_border 240         # box border (default: dark gray)
set -g @sidebar_color_border_active 117  # active group border (default: cyan)
set -g @sidebar_color_session 39         # session name (default: blue)
set -g @sidebar_color_agent_claude 174   # Claude brand color (default: terracotta)
set -g @sidebar_color_agent_codex 141    # Codex brand color (default: purple)
set -g @sidebar_color_text_active 255    # text for running/waiting (default: white)
set -g @sidebar_color_text_muted 250     # text for idle (default: light gray)
set -g @sidebar_color_wait_reason 221    # wait reason text (default: yellow)
set -g @sidebar_color_path 255           # directory path (default: white)
set -g @sidebar_color_selection 239      # selected row background (default: dark gray)
set -g @sidebar_color_branch 109        # git branch name (default: teal)

# Icons (Unicode glyphs; defaults keep the current look)
set -g @sidebar_icon_running ●           # running status icon
set -g @sidebar_icon_waiting ◐           # waiting status icon
set -g @sidebar_icon_idle ○              # idle status icon
set -g @sidebar_icon_error ✕             # error status icon
set -g @sidebar_icon_unknown ·           # unknown status icon

set -g @sidebar_prompt_lines 3           # max prompt display lines (default: 3)
set -g @sidebar_activity_lines 8         # max activity log entries (default: 8)

run-shell ~/.tmux/plugins/tmux-agent-sidebar/tmux-agent-sidebar.tmux
```

## Uninstalling

1. Remove the `set -g @plugin` (or `run-shell`) line from your `tmux.conf`
2. Remove hook entries from your Claude Code / Codex settings
3. Remove the plugin directory: `rm -rf ~/.tmux/plugins/tmux-agent-sidebar`
