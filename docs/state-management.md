# State Management Architecture

## State Scope & Update Frequency

Every piece of state belongs to one of three scopes: **Global** (shared across all sidebar instances via tmux variables), **Per-pane** (keyed by tmux pane ID), or **Local** (single sidebar process only). The table below shows where each field lives, how often it updates, and what triggers the update.

### Global State (synced via tmux global variables)

Stored in `GlobalState`. Written to tmux on change, reloaded on SIGUSR1.

| Field | Tmux Variable | Update Trigger | Description |
|-------|--------------|----------------|-------------|
| `status_filter` | `@sidebar_filter` | User input (left/right key) | Active status filter (All/Running/Waiting/Idle/Error) |
| `selected_pane_row` | `@sidebar_cursor` | User input (j/k key) | Cursor position in agent list |
| `repo_filter` | `@sidebar_repo_filter` | User input (repo popup) | Repository filter (All or specific repo) |

Each field has a corresponding `last_saved_*` to prevent sync conflicts — only overwrites tmux if the local write succeeded.

### Per-pane State (keyed by pane ID)

Written by `cli/hook.rs` on agent events, read by `query_sessions()` every **1 second**.

Each pane's runtime data is split into two buckets:

| Source | Update Trigger | Description |
|--------|----------------|-------------|
| tmux pane options | Event-driven + cleanup on agent exit | Agent type, status, cwd, permission mode, prompt, subagents, worktree, etc. |
| `PaneRuntimeState` in `AppState` | Refresh cycle + cleanup on agent exit | `ports`, `command`, `task_progress`, `task_dismissed_total`, `inactive_since` |

Pane options written to tmux:

| Tmux Option | Update Trigger | Description |
|-------------|----------------|-------------|
| `@pane_agent` | SessionStart | Agent type ("claude" / "codex") |
| `@pane_status` | Every event | Status ("running" / "waiting" / "idle" / "error") |
| `@pane_cwd` | SessionStart, CwdChanged | Working directory |
| `@pane_permission_mode` | SessionStart, hook event | Permission mode |
| `@pane_prompt` | UserPromptSubmit, Stop | Latest prompt or response text |
| `@pane_prompt_source` | UserPromptSubmit, Stop | "user" or "response" |
| `@pane_started_at` | UserPromptSubmit | Unix epoch when agent started |
| `@pane_attention` | SessionStart, Stop, StopFailure (clear); Notification, PermissionDenied, TeammateIdle (set) | "notification" or "clear" |
| `@pane_wait_reason` | StopFailure, PermissionDenied, TeammateIdle | Reason for waiting/error (`permission_denied`, `teammate_idle:<name>`, or error text) |
| `@pane_subagents` | SubagentStart/Stop | Comma-separated active subagent list |
| `@pane_worktree_name` | SessionStart | Worktree name (if applicable) |
| `@pane_worktree_branch` | SessionStart | Worktree branch (if applicable) |
| `@pane_session_id` | SessionStart, UserPromptSubmit, Notification, Stop, StopFailure, PermissionDenied, CwdChanged | Agent-reported session id (skipped when subagents are active) |

In-memory per-pane runtime state:

| Field | Update Frequency | Description |
|-------|-----------------|-------------|
| `pane_states[...].ports` | Every 10s (port scan) | Listening localhost ports detected from the pane process tree |
| `pane_states[...].command` | Every 10s (port scan) | Best-effort commandline for the pane process tree, with tmux command fallback in the UI |
| `pane_states[...].task_progress` | Every 1s (refresh cycle) | Parsed from activity log — task list per pane |
| `pane_states[...].task_dismissed_total` | On task completion | Tracks dismissed completed-task counts |
| `pane_states[...].inactive_since` | On status change | Debounce timestamp (3s grace before hiding tasks) |
| `pane_tab_prefs` | On user tab switch | Remembered bottom tab choice per pane |

Per-pane file-based state:

| File | Update Trigger | Read Frequency | Description |
|------|---------------|----------------|-------------|
| `/tmp/tmux-agent-activity_{pane_id}.log` | Each ActivityLog event | Every 1s | Tool usage log (`HH:MM\|tool\|label`), max 200 lines |

### Local State (single sidebar process only)

| Field | Update Frequency | Description |
|-------|-----------------|-------------|
| `sessions` | Every 1s | Full tmux session/window/pane hierarchy |
| `repo_groups` | Every 1s | Panes grouped by git repo root |
| `focused_pane_id` | Every 1s | Currently focused agent pane |
| `sidebar_focused` | Every 1s | Whether sidebar pane itself has focus |
| `now` | Every 1s | Current Unix epoch |
| `pane_row_targets` | Every 1s | Filtered pane list after applying filters |
| `focus` | On user input | UI focus: `Filter` / `Panes` / `ActivityLog` |
| `panes_scroll` | On user input / render | Agent list scroll position |
| `activity_scroll` | On user input / render | Activity log scroll position |
| `git_scroll` | On user input / render | Git status scroll position |
| `activity_entries` | Every 1s | Focused pane's activity entries (max 50) |
| `git` | Every 2s (bg thread) | Branch, diff stats, ahead/behind, PR number |
| `bottom_tab` | On user input / auto-switch | Current bottom panel tab |
| `line_to_row` | Every frame (render) | Rendered line → agent row mapping for click routing |
| `theme` | Once at startup | Color theme from tmux `@sidebar_color_*` variables |
| `popup` | On user input / render | `PopupState` enum: `None` / `Repo { selected, area }` / `Notices { area }`. Enforces "at most one popup open" via the type system |
| `notices.button_col` | Every frame (render) | Notices indicator button column position |
| `notices.missing_hook_groups` | Once at startup | Per-agent missing hook groups shown in the popup |
| `notices.claude_plugin_installed_version` | Once at startup | Version recorded in Claude Code's installed plugin registry |
| `notices.claude_settings_has_residual_hooks` | Once at startup | Whether `~/.claude/settings.json` still contains legacy Claude hook entries |
| `notices.claude_plugin_notice` | Once at startup | Derived Claude plugin notice shown in the popup |
| `notices.copy_targets` | Every frame (render) | Click targets for `[copy]` / `[prompt]` labels in the notices popup |
| `notices.copied_at` | On successful copy | Transient `[copied]` feedback state |
| `pending_osc52_copy` | On successful copy / frame flush | OSC 52 clipboard payload queued for terminal forwarding |
| `spinner_frame` | Every 200ms (animation) | Spinner animation frame counter |
| `icons` | Once at startup | `StatusIcons` theme (overridable via tmux options) |
| `tmux_pane` | Once at startup | This sidebar's own tmux pane ID |
| `activity_max_entries` | Once at startup | Max activity log entries to display |
| `prev_focused_pane_id` | Every 1s | Previous focused pane ID (for detecting focus changes) |
| `last_filter_click` | On user input | Last filter bar click timestamp (debounce) |
| `repo_button_col` | Every frame (render) | Repo filter button column position |
| `hyperlink_overlays` | Every frame (render) | OSC 8 hyperlink overlays to write after render |
| `seen_agent_panes` | Every 1s | Set of pane IDs that have been seen as agents |
| `port_scan_initialized` | Every 10s (port scan) | Whether the first port scan has completed |
| `last_port_refresh` | Every 10s (port scan) | Timestamp of the last port/command scan |
| `version_notice` | Once at startup (bg fetch) | GitHub release update notice, `None` when up-to-date |
| `session_names` | Periodic refresh | `session_id → session name` map from `~/.claude/sessions/*.json` |
| `last_session_refresh` | Periodic refresh | Timestamp of the last `session_names` refresh |

---

## Update Cycle Summary

```
┌─────────────────────────────────────────────────────────────┐
│  Every frame (~200ms)                                       │
│  line_to_row, scroll dimensions, spinner animation          │
├─────────────────────────────────────────────────────────────┤
│  Every 1s (refresh cycle)                                   │
│  sessions, repo_groups, focused_pane_id, pane_row_targets, │
│  activity_entries, pane_states.task_progress                │
├─────────────────────────────────────────────────────────────┤
│  Every 10s (port scan)                                      │
│  pane_states.ports, agent liveness cleanup                  │
├─────────────────────────────────────────────────────────────┤
│  Once at startup                                             │
│  theme, bottom_panel_height, notices.claude_plugin_*,       │
│  notices.claude_settings_has_residual_hooks,                │
│  notices.claude_plugin_notice, notices.missing_hook_groups  │
├─────────────────────────────────────────────────────────────┤
│  Every 2s (git background thread)                           │
│  git (branch, diff, ahead/behind, PR)                       │
├─────────────────────────────────────────────────────────────┤
│  On SIGUSR1 (tmux focus change)                             │
│  GlobalState reloaded from tmux variables                   │
├─────────────────────────────────────────────────────────────┤
│  Event-driven (agent hooks)                                 │
│  @pane_* tmux options, activity log files                   │
├─────────────────────────────────────────────────────────────┤
│  On user input                                              │
│  focus, scroll offsets, bottom_tab, GlobalState fields,     │
│  popup (PopupState enum)                                    │
├─────────────────────────────────────────────────────────────┤
│  Every frame (render)                                       │
│  line_to_row, popup.area (Repo/Notices variants),           │
│  notices.button_col, notices.copy_targets, hyperlink_overlays
└─────────────────────────────────────────────────────────────┘
```

---

## Data Flow

```
Agent hooks (hook.sh)
  → CLI `hook` subcommand (cli/hook.rs)
    → resolve_adapter() (event.rs) → adapter.parse() → AgentEvent
    → handle_event() writes @pane_* tmux options + /tmp activity log files
                        ↓
TUI main loop (main.rs)
  → startup plugin-state reads (cli/plugin_state.rs)
    → installed_plugins.json / ~/.claude/settings.json
    → initializes Claude notices state once
                        ↓
  → refresh() every 1s
    → query_sessions() (tmux.rs)     ← reads @pane_* via `tmux list-panes -a`
    → group_panes_by_repo() (group.rs)
    → rebuild_row_targets()          ← applies GlobalState filters
    → refresh_activity_data()        ← reads /tmp activity logs
    → refresh_task_progress()        ← updates PaneRuntimeState.task_progress
    → refresh_port_data()            ← updates PaneRuntimeState.ports
    → scan_session_process_snapshot() ← detects dead panes and clears stale tmux metadata
                        ↓
  → git_rx.try_recv()                ← receives GitData from background thread
  → notices popup render/copy state  ← derived from AppState plugin fields
                        ↓
  → ui::draw() renders frame         ← reads all AppState fields
```

---

## Key Types

```rust
enum Focus { Filter, Panes, ActivityLog }
enum StatusFilter { All, Running, Waiting, Idle, Error }
enum RepoFilter { All, Repo(String) }
enum BottomTab { Activity, GitStatus }
enum PaneStatus { Running, Waiting, Idle, Error, Unknown }
enum AgentType { Claude, Codex, Unknown }
enum PermissionMode { Default, Plan, AcceptEdits, Auto, DontAsk, BypassPermissions }

struct ScrollState {
    offset: usize,
    total_lines: usize,
    visible_height: usize,
}

struct HyperlinkOverlay {
    x: u16,
    y: u16,
    text: String,
    url: String,
}

struct PaneRuntimeState {
    ports: Vec<u16>,
    command: Option<String>,
    task_progress: Option<TaskProgress>,
    task_dismissed_total: Option<usize>,
    inactive_since: Option<u64>,
}
```

---

## State Invariants

1. `selected_pane_row` is always < `pane_row_targets.len()` — clamped in `rebuild_row_targets()`
2. `activity_entries` contains only the focused pane's entries — cleared on focus change
3. Tab preferences persist per pane ID in `pane_tab_prefs` — restored on focus change
4. Git fetching respects the `git_tab_active` flag — stops when tab is hidden
5. Task progress has a 3-second debounce — prevents flicker when agent briefly pauses
6. Global state syncs via tmux variables — enables coordination across sidebar instances
7. Scroll positions are independent per panel — agents, activity, git each have their own `ScrollState`
8. `line_to_row` is rebuilt every frame — ensures accurate click routing
9. Pane runtime state is pruned when the pane disappears — prevents stale per-pane ports and task progress from surviving after the agent is gone
10. Hook-based cleanup wins when available; pid-based cleanup is a slower fallback that removes panes when the agent process is gone but the hook did not fire
