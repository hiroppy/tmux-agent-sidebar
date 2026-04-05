# tmux-agent-sidebar

A tmux sidebar that aggregates all AI coding agents (Claude Code, Codex) across every window and session into a single, unified dashboard.

Displays real-time statuses, user prompts, elapsed time, git info, subagent trees, task progress, and a live activity log — all without switching windows.

<!-- TODO: screenshot -->

## Features

- **Cross-session monitoring** — All agents across all tmux sessions and windows in one view
- **Repository grouping** — Agents are grouped by git repository; worktrees share the same group
- **Pane navigation** — Select an agent and press Enter to jump to its pane (across windows)
- **Instant refresh** — Pane focus changes trigger SIGUSR1 for immediate UI update
- **Auto sidebar** — Automatically opens on new windows, auto-closes when only sidebar remains
- **Per-pane tab memory** — Remembers your bottom tab (Activity/Git) choice per pane
- **Responsive width** — Supports percentage-based width (e.g. `15%`) that adapts to window size
- **Subagent tree** — Visualizes parallel subagents with tree connectors (`├`, `└`)
- **Task progress** — Tracks task completion with smart debouncing to prevent UI flicker
- **Activity log** — 20+ tool types with distinct color coding for visual scanning
- **Git integration** — Branch, ahead/behind arrows, diff stats, PR number, and file-level changes
- **Status filter** — Filter agents by status (All / Running / Waiting / Idle / Error) with live counts
- **Permission badges** — Color-coded `auto` / `plan` / `!` badges always visible

## Requirements

- tmux 3.0+
- [Rust](https://rustup.rs/) (only if building from source)
- [GitHub CLI](https://cli.github.com/) (optional, for PR number display in Git tab)

## Installation

### TPM (recommended)

Add the plugin to your `tmux.conf`:

```tmux
set -g @plugin 'hiroppy/tmux-agent-sidebar'
run '~/.tmux/plugins/tpm/tpm'
```

Press `prefix + I` to install. On first run, an install wizard will prompt you to download a pre-built binary or build from source.

### Manual

Clone the repository:

```sh
git clone https://github.com/hiroppy/tmux-agent-sidebar.git ~/.tmux/plugins/tmux-agent-sidebar
```

Then add to your `tmux.conf`:

```tmux
run-shell ~/.tmux/plugins/tmux-agent-sidebar/tmux-agent-sidebar.tmux
```

Install the binary using one of the following methods:

**Option A: Download pre-built binary**

```sh
# macOS (Apple Silicon)
curl -fSL https://github.com/hiroppy/tmux-agent-sidebar/releases/latest/download/tmux-agent-sidebar-darwin-aarch64 \
  -o ~/.tmux/plugins/tmux-agent-sidebar/bin/tmux-agent-sidebar
chmod +x ~/.tmux/plugins/tmux-agent-sidebar/bin/tmux-agent-sidebar
```

**Option B: Build from source (requires Rust)**

```sh
cd ~/.tmux/plugins/tmux-agent-sidebar
cargo build --release
```

Reload tmux config with `prefix + r`.

### Setting Up Agent Hooks

The sidebar receives status updates through agent hooks. Add the following hook configurations to your agent settings.

<details>
<summary>Claude Code</summary>

Add to your Claude Code hooks configuration (e.g. `~/.claude/settings.json`):

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

<details>
<summary>Codex</summary>

Create or edit `~/.codex/hooks.json`:

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
    ]
  }
}
```

> **Note:** Codex does not support `Notification` or `PostToolUse` hooks, so waiting status and activity logging are unavailable.

</details>

## Usage

### Toggle Sidebar

| Key | Action |
|---|---|
| `prefix + e` | Toggle sidebar (default keybinding) |

### Sidebar Navigation

| Key | Action |
|---|---|
| `j` / `Down` | Move selection down (filter → agents → bottom panel) |
| `k` / `Up` | Move selection up |
| `h` / `Left` | Previous filter (when on filter bar) |
| `l` / `Right` | Next filter (when on filter bar) |
| `Enter` | Activate selected pane |
| `Tab` | Cycle status filter (All → Running → Waiting → Idle → Error) |
| `Shift+Tab` | Switch bottom panel tab (Activity / Git) |
| `Esc` | Return focus to agents panel |
| Mouse click | Click agent to jump to its pane, click filter bar to select filter |

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

- **Waiting status persists after tool approval (Claude Code)** — After approving a permission prompt, the status stays `waiting` until the next event. This is a limitation of the Claude Code hook system.
- **No waiting status for Codex** — Codex does not support the `Notification` hook, so the `waiting` state is unavailable.

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
set -g @sidebar_prompt_lines 3           # max prompt display lines (default: 3)
set -g @sidebar_activity_lines 8         # max activity log entries (default: 8)

run-shell ~/.tmux/plugins/tmux-agent-sidebar/tmux-agent-sidebar.tmux
```

## Uninstalling

1. Remove the `set -g @plugin` (or `run-shell`) line from your `tmux.conf`
2. Remove hook entries from your Claude Code / Codex settings
3. Remove the plugin directory: `rm -rf ~/.tmux/plugins/tmux-agent-sidebar`
