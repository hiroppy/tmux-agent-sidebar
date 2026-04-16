<h1 align="center">tmux-agent-sidebar</h1>

<p align="center">A tmux sidebar that monitors all AI coding agents (Claude Code, Codex) across all sessions and windows — statuses, prompts, Git info, activity logs, and more in one place.</p>

<p align="center"><img src="assets/main.png" alt="main" /></p>

## Features

- **Cross-session monitoring** — Shows all agents from every tmux session and window in one sidebar
- **Activity log** — Streams each tool invocation (Read, Edit, Bash, etc.) per agent in real time
- **Task & subagent tracking** — Displays task progress (e.g. `3/7`) and spawned subagents as a parent-child tree
- **Git integration** — Shows branch name, ahead/behind counts, PR number (`gh`), and per-file diff stats
- **Worktree-aware grouping** — Groups agents by the same repo, including worktrees, so related panes stay together
- **Spawn & remove worktrees from the sidebar** — Press `n` (or click `+` next to a repo header) to create a new `git worktree`, open a tmux window in it, and launch an agent in one step. Remove it later with `x`, or click the red `×` next to the branch on any spawn-created row
- **Pane metadata** — Shows listening localhost ports and execution command info for each pane
- **Desktop notifications** — Hook-driven OS alerts for task completion/failure and permission prompts

## Agent Pane

<table>
  <tr>
    <td width="55%"><img src="assets/agent-pane.png" alt="Agent pane" /></td>
    <td valign="top">
      <ul>
        <li><b>Status icon</b>
          <ul>
            <li><code>●</code> running, <code>◐</code> waiting, <code>○</code> idle, <code>✕</code> error</li>
          </ul>
        </li>
        <li><b>Agent color</b>
          <ul>
            <li>Claude (terracotta), Codex (purple), <code>/rename</code></li>
          </ul>
        </li>
        <li><b>Permission badge</b>
          <ul>
            <li><code>plan</code>, <code>edit</code>, <code>auto</code>, <code>!</code></li>
          </ul>
        </li>
        <li><b>Session name</b>
          <ul><li>the tmux session the pane belongs to</li></ul>
        </li>
        <li><b>+ marker</b>
          <ul><li>indicates a git worktree</li></ul>
        </li>
        <li><b>Branch</b>
          <ul><li>the current Git branch for the pane's cwd</li></ul>
        </li>
        <li><b>Elapsed time</b>
          <ul><li>time since the last user prompt</li></ul>
        </li>
        <li><b>Task progress</b>
          <ul><li>e.g. <code>3/7</code>, synchronized from the agent's task list</li></ul>
        </li>
        <li><b>Subagent tree</b>
          <ul><li>parent-child branches for spawned subagents</li></ul>
        </li>
        <li><b>Listening ports</b>
          <ul><li>localhost ports the pane's process is listening on</li></ul>
        </li>
        <li><b>Response arrow (▷)</b>
          <ul><li>preview of the latest agent response</li></ul>
        </li>
        <li><b>Prompt text</b>
          <ul><li>latest user prompt</li></ul>
        </li>
        <li><b>Wait reason</b>
          <ul><li>why the agent is waiting</li></ul>
        </li>
      </ul>
    </td>
  </tr>
</table>

## Requirements

- tmux 3.0+
- [TPM](https://github.com/tmux-plugins/tpm) (for plugin installation)
- [GitHub CLI](https://cli.github.com/) (optional, for displaying PR numbers in the Git tab)
- [Rust](https://rustup.rs/) (only if building from source)

## Setting Up

### 1. Installation

**Option A: Installation with TPM (recommended).**

Add the plugin to your `tmux.conf`:

```tmux
set -g @plugin 'hiroppy/tmux-agent-sidebar'
```

Reload tmux.conf (`tmux source ~/.tmux.conf`), then press `prefix + I` to install. On the first run, an install wizard prompts you to download a pre-built binary or build from source.

To update later, press `prefix + U` in TPM's plugin list and select `tmux-agent-sidebar`. The install wizard runs again if the bundled binary has changed.

<details>
<summary>Option B: Manual</summary>

1. Clone the repository:

```sh
git clone https://github.com/hiroppy/tmux-agent-sidebar.git ~/.tmux/plugins/tmux-agent-sidebar
```

2. Add the plugin to your `tmux.conf`:

```tmux
run-shell ~/.tmux/plugins/tmux-agent-sidebar/tmux-agent-sidebar.tmux
```

3. Install the binary using one of the following methods:

```sh
# macOS (Apple Silicon)
curl -fSL https://github.com/hiroppy/tmux-agent-sidebar/releases/latest/download/tmux-agent-sidebar-darwin-aarch64 \
  -o ~/.tmux/plugins/tmux-agent-sidebar/bin/tmux-agent-sidebar
chmod +x ~/.tmux/plugins/tmux-agent-sidebar/bin/tmux-agent-sidebar
cd ~/.tmux/plugins/tmux-agent-sidebar
cargo build --release
```

</details>

### 2. Reload tmux config

After updating `tmux.conf`, press `prefix + r` to reload the config.

### 3. Agent Hooks

The sidebar receives status updates through agent hooks. Add the following hook definitions to your agent settings.

#### 3.1 Claude Code

The repository ships as a Claude Code plugin, so the hooks register themselves.

Inside Claude Code, register the marketplace and install the plugin:

```sh
/plugin marketplace add ~/.tmux/plugins/tmux-agent-sidebar
/plugin install tmux-agent-sidebar@hiroppy
```

Either form wires up the Claude Code hooks. Run `/reload-plugins` (or restart Claude Code) to activate them.

<details>
<summary>
If your environment can' use plugin, you will be able to register hooks to your settings.json using below prompt.
</summary>

```
Run ~/.tmux/plugins/tmux-agent-sidebar/target/release/tmux-agent-sidebar setup claude (fall back to ~/.tmux/plugins/tmux-agent-sidebar/bin/tmux-agent-sidebar if that path is missing). Add these hooks to ~/.claude/settings.json. If hooks already exist, merge them without making destructive changes.
```

</details>

#### 3.2 Codex

1. Open a Codex pane in tmux and focus it.
2. Press `prefix + e` to toggle the sidebar. A yellow `ⓘ` badge appears in the top row of the sidebar when required hooks are missing.
3. Click `ⓘ`, then click `[copy]` next to `codex` in the Notices popup.
4. Switch back to the Codex pane and paste. Codex will run `tmux-agent-sidebar setup codex` and merge the hooks into `~/.codex/hooks.json`.

## Keybindings

| Key | Action |
|---|---|
| `prefix + e` | Toggle sidebar (default, customizable) |
| `prefix + E` | Toggle sidebar in all windows (default, customizable) |
| `j` / `Down` | Move selection down (filter → agents → bottom panel) |
| `k` / `Up` | Move selection up |
| `h` / `Left` | Previous status filter when the filter bar is focused |
| `l` / `Right` | Next status filter when the filter bar is focused |
| `r` | Open repo filter popup (filter bar only) |
| `n` | Spawn a new worktree + agent for the selected row's repo |
| `x` | Remove the selected spawn-created pane (opens the close modal) |
| `Enter` | Jump to the selected agent's pane / confirm the repo popup |
| `Tab` | Cycle status filter (All → Running → Waiting → Idle → Error) |
| `Shift+Tab` | Switch bottom panel tab (Activity / Git) |
| `Esc` | Return focus to the agents panel / close the repo popup |
| Mouse click `+` | Open the spawn modal for that repo (right edge of each repo header) |
| Mouse click `×` | Open the close-pane modal for that spawn-created worktree (red `×` next to the branch) |
| Mouse click | Jump to an agent's pane / filter by status / open the repo popup |

### Spawn worktree modal

Opened with `n` or by clicking the `+` button next to a repo header.

| Key | Action |
|---|---|
| Text keys | Type the name (used as the branch slug and tmux window name, e.g. `add login form` → `agent/add-login-form`) |
| `↑` / `↓` / `Tab` / `Shift+Tab` | Move focus between `NAME` / `AGENT` / `MODE` fields |
| `←` / `→` | Cycle the value when the agent or mode field has focus |
| `Enter` | Create the worktree + window and launch the agent |
| `Esc` / click outside | Cancel |

### Close pane modal

Opened with `x` on a spawn-created pane.

| Key | Action |
|---|---|
| `y` / `Enter` | Close the tmux window, remove the git worktree (`--force`), **and** delete the branch the spawn created (`git branch -D`) |
| `c` | Close the tmux window only, keep the worktree and branch on disk |
| `n` / `Esc` | Cancel |

Branches are force-deleted because the sidebar auto-generates them under the `agent/` prefix for short-lived explorations; squash/rebase-merged work would otherwise be refused by the non-forced `git branch -d` check. Recover via `git reflog` if needed.


## Feature Support by Agent

| Feature | Claude Code | Codex | Notes |
|---|---|---|---|
| Status tracking (running / idle / error) | :white_check_mark: | :white_check_mark: | Driven by `SessionStart` / `UserPromptSubmit` / `Stop` |
| Prompt text display | :white_check_mark: | :white_check_mark: | Saved from `UserPromptSubmit` |
| Response text display (`▷ ...`) | :white_check_mark: | :white_check_mark: | Populated from `Stop` payload |
| Waiting status + wait reason | :white_check_mark: | :x: | Populated from `Notification`, `PermissionDenied`, and `TeammateIdle` (all Claude-only) |
| API failure reason display | :white_check_mark: | :x: | `StopFailure` is wired only for Claude |
| Permission badge | :white_check_mark: (`plan` / `edit` / `auto` / `!`) | :white_check_mark: (`auto` / `!` only) | Codex badges are inferred from process arguments |
| Git branch display | :white_check_mark: | :white_check_mark: | Uses the pane `cwd`; Claude updates dynamically via `CwdChanged` |
| Elapsed time | :white_check_mark: | :white_check_mark: | Since the last prompt |
| Task progress | :white_check_mark: | :x: | Requires `PostToolUse`; Codex fires `PostToolUse` only for `Bash`, so task progress from tools is unavailable |
| Task lifecycle notifications | :white_check_mark: | :white_check_mark: (`Stop` only) | `Stop` desktop notifications fire for both. `Notification`, `TaskCompleted`, `StopFailure`, and `PermissionDenied` are Claude-only because Codex does not emit those hooks |
| Subagent display | :white_check_mark: | :x: | Requires `SubagentStart` / `SubagentStop` |
| Activity log | :white_check_mark: | :white_check_mark: (Bash only) | Codex's `PostToolUse` fires only for `Bash` tool calls; `Read`/`Edit`/`Write`/`Grep`/`Glob`/etc. are not reported |
| Worktree lifecycle tracking | :white_check_mark: | :x: | Requires `WorktreeCreate` / `WorktreeRemove` |

### Known Limitations

- **Waiting status (Claude Code)** — After you approve a permission prompt, the status stays `waiting` until the next hook event fires. This is a limitation of the Claude Code hook system.
- **Codex hook coverage** — Codex emits `SessionStart`, `UserPromptSubmit`, `Stop`, and `PostToolUse`. `PostToolUse` is limited to the `Bash` tool (Codex's schema types `tool_input` as `{ command: string }`), so the Codex activity log shows only Bash commands. Waiting status, task progress, subagent display, and worktree tracking remain unavailable.
- **Desktop notifications** — macOS requires `osascript`; Linux requires `notify-send`. If those commands are missing, notifications stay silent even when `@sidebar_notifications` is enabled. `Stop` notifications fire for both Claude Code and Codex; `Notification`, `TaskCompleted`, `StopFailure`, and `PermissionDenied` fire only for Claude Code (Codex does not emit those hooks).

## Customization

Most options can be set **before** loading the plugin in your `tmux.conf`:

```tmux
# Sidebar
set -g @sidebar_key T                    # keybinding (default: e)
set -g @sidebar_key_all Y                # keybinding for all windows (default: E)
set -g @sidebar_width 32                 # width in columns or % (default: 15%)
set -g @sidebar_bottom_height 20         # bottom panel height in lines (default: 20, 0 to hide)
set -g @sidebar_auto_create off          # disable auto-create on new windows (default: on)
set -g @sidebar_notifications off        # desktop notifications for task completion/failure and permission prompts (default: on)
set -g @sidebar_notifications_events "stop,notification" # limit desktop notifications to selected hook events (default: all)

# Spawn worktree modal defaults (optional)
set -g @agent-sidebar-default-agent codex  # agent launched by `n` (default: claude)
set -g @agent-sidebar-branch-prefix wip/   # branch prefix for new worktrees (default: agent/)

# Colors (256-color palette numbers) — all defaults live in src/ui/colors.rs
set -g @sidebar_color_all 111            # selected "all" filter icon (default: 111 sky blue)
set -g @sidebar_color_running 114        # selected running filter icon and running pane status (default: 114 green)
set -g @sidebar_color_waiting 221        # selected waiting filter icon, waiting pane status, version banner (default: 221 yellow)
set -g @sidebar_color_idle 110           # selected idle filter icon and idle pane status (default: 110 soft blue)
set -g @sidebar_color_error 167          # selected error filter icon and error pane status (default: 167 soft red)
set -g @sidebar_color_filter_inactive 245 # unselected status filter icons and zero counts (default: 245 mid gray)
set -g @sidebar_color_border 240         # unfocused panel borders and tab separators (default: 240 dark gray)
set -g @sidebar_color_accent 153         # active pane marker, focused repo header, focused bottom panel border, repo popup border (default: 153 pale sky blue)
set -g @sidebar_color_session 39         # session name (default: 39 blue)
set -g @sidebar_color_agent_claude 174   # Claude brand color (default: 174 terracotta)
set -g @sidebar_color_agent_codex 141    # Codex brand color (default: 141 purple)
set -g @sidebar_color_text_active 255    # primary text (active rows, counts, filtered repo label) (default: 255 white)
set -g @sidebar_color_text_muted 252     # secondary text (tree branches, empty-state messages, inactive bottom tabs, activity log labels) (default: 252 light gray)
set -g @sidebar_color_text_inactive 244  # body text of unfocused pane rows (prompt/response, idle hint) (default: 244 mid gray)
set -g @sidebar_color_port 246           # port numbers (default: 246 light gray)
set -g @sidebar_color_wait_reason 221    # wait reason text (default: 221 yellow)
set -g @sidebar_color_selection 239      # selected row background (default: 239 slightly lighter dark gray)
set -g @sidebar_color_branch 109         # git branch name (default: 109 teal)
set -g @sidebar_color_task_progress 223   # task progress summary (default: 223 pale yellow)
set -g @sidebar_color_subagent 73         # subagent tree (default: 73 green)
set -g @sidebar_color_commit_hash 221     # commit hash (default: 221 yellow)
set -g @sidebar_color_diff_added 114      # added diff lines (default: 114 green)
set -g @sidebar_color_diff_deleted 174    # deleted diff lines (default: 174 terracotta)
set -g @sidebar_color_file_change 221     # file change stats (default: 221 yellow)
set -g @sidebar_color_pr_link 117         # PR link / number (default: 117 blue)
set -g @sidebar_color_section_title 109   # section titles (default: 109 teal)
set -g @sidebar_color_activity_timestamp 109 # activity timestamps (default: 109 teal)
set -g @sidebar_color_response_arrow 81   # response arrow (default: 81 bright cyan)

# Icons (Unicode glyphs; defaults keep the current look)
set -g @sidebar_icon_all ≡               # status filter bar "all" icon
set -g @sidebar_icon_running ●           # running status icon
set -g @sidebar_icon_waiting ◐           # waiting status icon
set -g @sidebar_icon_idle ○              # idle status icon
set -g @sidebar_icon_error ✕             # error status icon
set -g @sidebar_icon_unknown ·           # unknown status icon

run-shell ~/.tmux/plugins/tmux-agent-sidebar/tmux-agent-sidebar.tmux
```

## Desktop Notifications

Desktop notifications are enabled by default.

Enable them with tmux global options:

```tmux
set -g @sidebar_notifications on
```

Supported events:

- `stop` — assistant finished responding (`Stop` hook; Claude Code and Codex)
- `notification` — permission prompt or other attention request (`Notification` hook; Claude only)
- `task_completed` — subagent / Task tool completion (`TaskCompleted` hook; Claude only)
- `stop_failure` — assistant ended with an error (`StopFailure` hook; Claude only)
- `permission_denied` — permission explicitly denied (`PermissionDenied` hook; Claude only)

Restrict which events fire notifications with `@sidebar_notifications_events` (comma-separated event names; `all` or unset = every event):

```tmux
set -g @sidebar_notifications_events "stop,notification"  # drop subagent + error notifications
set -g @sidebar_notifications_events all                  # explicit "fire everything" (default)
```

Setting an empty value disables every event without touching the master `@sidebar_notifications` switch.

Delivery depends on the platform:

- macOS uses `osascript`
- Linux uses `notify-send`
- Windows is not supported yet

If the required notification command is missing, the feature quietly disables itself.

Notifications are titled `repo / agent` when repo information is available, with the event summary in the body.

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

If you build your own notifications, the same pane options can be used to drive them.

## Uninstalling

1. Remove the `set -g @plugin` (or `run-shell`) line from your `tmux.conf`
2. Remove hook entries or plugins from your Claude Code / Codex settings
3. Remove the plugin directory: `rm -rf ~/.tmux/plugins/tmux-agent-sidebar`

## Development

Symlink the plugin directory to your working copy so that builds are picked up without copying artifacts. If TPM already cloned the plugin, remove it first:

```sh
rm -rf ~/.tmux/plugins/tmux-agent-sidebar
ln -s <path-to-this-repo> ~/.tmux/plugins/tmux-agent-sidebar
```

Then build — the binary that your local tmux sidebar loads is replaced in place. Toggle the sidebar off → on to pick up the new build.

```sh
cargo build --release
# or, enable the `debug` feature to force-display notices (version, missing hooks, claude plugin)
cargo build --release --features debug
```

### Claude Code plugin

This repository is itself a Claude Code marketplace (see `.claude-plugin/marketplace.json`). Because `~/.tmux/plugins/tmux-agent-sidebar` is symlinked to the repo, the same install flow from [3.1 Claude Code](#31-claude-code) already points Claude Code at your working copy:

```
/plugin marketplace add ~/.tmux/plugins/tmux-agent-sidebar
/plugin install tmux-agent-sidebar@hiroppy
```

After editing plugin files, pick up changes without reinstalling:

- **`hooks/hooks.json`, `.claude-plugin/plugin.json`, `hook.sh`** — run `/reload-plugins` in Claude Code (or restart it).
- **Rust sources** — `cargo build --release`, then toggle the sidebar off → on.

To iterate from a git worktree, register the worktree path as its own marketplace:

```
/plugin marketplace add <worktree-path>
/plugin install tmux-agent-sidebar@hiroppy
```
