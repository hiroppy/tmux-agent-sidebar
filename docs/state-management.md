# State Management Architecture

## State Scope & Update Frequency

Every piece of state belongs to one of three scopes: **Global** (shared across all sidebar instances via tmux variables), **Per-pane** (keyed by tmux pane ID), or **Local** (single sidebar process only). The table below shows where each field lives, how often it updates, and what triggers the update.

### Global State (synced via tmux global variables)

Stored in `GlobalState`. Written to tmux on change, with the cursor save
debounced briefly so selection changes do not block redraw/input handling;
reloaded on SIGUSR1.

| Field | Tmux Variable | Update Trigger | Description |
|-------|--------------|----------------|-------------|
| `status_filter` | `@sidebar_filter` | User input (left/right key) | Active status filter (All/Running/Waiting/Idle/Error) |
| `selected_pane_row` | `@sidebar_cursor` | User input (j/k key); tmux write flushed after a short debounce | Cursor position in agent list |
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

In-memory per-pane runtime state. Every field lives inside
`PaneRuntimeState` so the whole record is dropped together when its
pane disappears (`prune_pane_states_to_current_panes`).

| Field | Update Frequency | Description |
|-------|-----------------|-------------|
| `pane_states.map[...].ports` | Every 10s (port scan) | Listening localhost ports detected from the pane process tree |
| `pane_states.map[...].command` | Every 10s (port scan) | Best-effort commandline for the pane process tree, with tmux command fallback in the UI |
| `pane_states.map[...].task_progress` | Every 1s (refresh cycle) | Parsed from activity log — task list per pane |
| `pane_states.map[...].task_dismissed_total` | On task completion | Tracks dismissed completed-task counts |
| `pane_states.map[...].inactive_since` | On status change | Debounce timestamp (3s grace before hiding tasks) |
| `pane_states.map[...].tab_pref` | On user tab switch | Remembered bottom tab choice per pane (cleared on relaunch) |
| `pane_states.map[...].task_progress_log_mtime` | Every 1s (refresh cycle) | mtime of the task-progress log last parsed; skips re-parsing when unchanged |

Per-pane file-based state:

| File | Update Trigger | Read Frequency | Description |
|------|---------------|----------------|-------------|
| `/tmp/tmux-agent-activity_{pane_id}.log` | Each ActivityLog event | Every 1s | Tool usage log (`HH:MM\|tool\|label`), max 200 lines |

### Local State (single sidebar process only)

| Field | Update Frequency | Description |
|-------|-----------------|-------------|
| `repo_groups` | Every 1s | Panes grouped by git repo root (built directly from `tmux::query_sessions()` output, not stored separately as a session list) |
| `focus_state.focused_pane_id` | Every 1s, plus immediately on user-initiated pane jumps | Currently focused agent pane |
| `focus_state.sidebar_focused` | Every 1s | Whether sidebar pane itself has focus |
| `focus_state.focus` | On user input | UI focus: `Filter` / `Panes` / `ActivityLog`; input also triggers an immediate redraw so focus changes appear without waiting for the next poll tick |
| `focus_state.prev_focused_pane_id` | Every 1s | Previous focused pane ID (for detecting focus changes) |
| `now` | Every 1s | Current Unix epoch |
| `scrolls.panes` | On user input / render | Agent list scroll position |
| `scrolls.git` | On user input / render | Git status scroll position |
| `activity.scroll` | On user input / render | Activity log scroll position |
| `activity.entries` | Every 1s | Focused pane's activity entries (max 50) |
| `activity.max_entries` | Once at startup | Max activity log entries to display |
| `activity.log_cache` | Every 1s | `(focused_pane_id, mtime)` of the last-rendered activity log; skips re-reads when unchanged |
| `git` | Every 2s (bg thread) | Branch, diff stats, ahead/behind, PR number |
| `bottom_tab` | On user input / auto-switch | Current bottom panel tab |
| `theme` | Once at startup | Color theme from tmux `@sidebar_color_*` variables |
| `popup` | On user input / render | `PopupState` enum: `None` / `Repo { selected, area }` / `Notices { area }`. Enforces "at most one popup open" via the type system |
| `layout` | Every frame (render) | `FrameLayout` sub-struct bundling the ephemeral fields the UI rewrites every frame for click hit-testing: `pane_row_targets`, `line_to_row`, `repo_button_col`, `repo_spawn_targets`, `spawn_remove_targets`, `hyperlink_overlays` |
| `notices` | Once at startup / on copy | `NoticesState` sub-struct: `button_col`, `missing_hook_groups`, `claude_plugin_installed_version`, `claude_settings_has_residual_hooks`, `claude_plugin_notice`, `copy_targets`, `copied_at` |
| `timers` | Refresh cycle / on user input | `RefreshTimers` sub-struct gating periodic work: `last_filter_click` (debounce), `last_port_refresh`, `port_scan_initialized` |
| `pending_osc52_copy` | On successful copy / frame flush | OSC 52 clipboard payload queued for terminal forwarding |
| `spinner_frame` | Every 200ms (animation) | Spinner animation frame counter |
| `icons` | Once at startup | `StatusIcons` theme (overridable via tmux options) |
| `tmux_pane` | Once at startup | This sidebar's own tmux pane ID |
| `pane_states.seen` | Every 1s | Set of pane IDs that have been seen as agents (bundled with `pane_states.map` under the `PaneRuntimeMap` wrapper) |
| `version_notice` | Once at startup (bg fetch) | GitHub release update notice, `None` when up-to-date |
| `sessions.names` | Every 10s (background thread) | `session_id → session name` map; scanned by `session_poll_loop` in `main.rs` so the TUI thread never blocks on filesystem I/O |
| `sessions.dirty` | On session map refresh / application tick | Marks the session map as changed so the per-pane session label walk only runs when needed |

---

## Update Cycle Summary

```
┌─────────────────────────────────────────────────────────────┐
│  Every frame (~200ms)                                       │
│  layout.* (rebuilt by ui::draw), spinner animation          │
├─────────────────────────────────────────────────────────────┤
│  Every 1s (refresh cycle)                                   │
│  repo_groups, focus_state.focused_pane_id,                  │
│  layout.pane_row_targets, activity.entries,                 │
│  pane_states.map[..].task_progress                          │
├─────────────────────────────────────────────────────────────┤
│  Every 10s (port scan, background)                          │
│  pane_states.map[..].ports, agent liveness cleanup          │
├─────────────────────────────────────────────────────────────┤
│  Every 10s (session_names background thread)                │
│  sessions.names map populated by session_poll_loop          │
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
│  focus_state.focus, scrolls.*, activity.scroll, bottom_tab, │
│  GlobalState fields, popup (PopupState enum),               │
│  timers.last_filter_click,                                  │
│  immediate selection / active-pane redraw                   │
├─────────────────────────────────────────────────────────────┤
│  Every frame (render)                                       │
│  layout.line_to_row, popup.area (Repo/Notices variants),    │
│  notices.button_col, notices.copy_targets,                  │
│  layout.hyperlink_overlays                                  │
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

/// At-most-one popup state. The enum encodes both which popup is open
/// and its per-popup data so the invariant is checked by the type system.
enum PopupState {
    None,
    Repo { selected: usize, area: Option<Rect> },
    Notices { area: Option<Rect> },
}

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
    tab_pref: Option<BottomTab>,
    task_progress_log_mtime: Option<SystemTime>,
}

/// Wraps `PaneRuntimeState` per pane plus the set of pane IDs that
/// have been seen as agents. Methods delegate to the underlying
/// `HashMap`; `seen` is read/written alongside `map` during refresh.
struct PaneRuntimeMap {
    map: HashMap<String, PaneRuntimeState>,
    seen: HashSet<String>,
}

/// Focus-related fields grouped so UI code can pass them as a single
/// sub-struct rather than juggling four flat fields.
struct FocusState {
    sidebar_focused: bool,
    focus: Focus,
    focused_pane_id: Option<String>,
    prev_focused_pane_id: Option<String>,
}

/// Non-activity scrolls (the agent list and the git bottom panel).
/// Activity's scroll lives inside `ActivityState` because it pairs
/// with the activity entries buffer.
struct ScrollStates {
    panes: ScrollState,
    git: ScrollState,
}

/// Activity-log snapshot for the focused pane plus cache metadata so
/// the polling tick can skip redundant file reads.
struct ActivityState {
    entries: Vec<ActivityEntry>,
    scroll: ScrollState,
    max_entries: usize,
    log_cache: Option<(String, SystemTime)>,
}

/// Session-name map scanned by a background thread so the TUI thread
/// never blocks on `~/.claude/sessions/*.json` reads.
struct SessionNamesState {
    names: HashMap<String, String>,
    dirty: bool,
}

/// Frame-scoped render output cached for click hit-testing. Rewritten
/// every frame by the UI layer; consumed by mouse/keyboard handlers
/// before the next render.
struct FrameLayout {
    pane_row_targets: Vec<RowTarget>,
    line_to_row: Vec<Option<usize>>,
    repo_button_col: Option<u16>,
    repo_spawn_targets: Vec<RepoSpawnTarget>,
    spawn_remove_targets: Vec<SpawnRemoveTarget>,
    hyperlink_overlays: Vec<HyperlinkOverlay>,
}

/// Periodic-refresh bookkeeping. session_names refresh is intentionally
/// NOT here — it lives in a dedicated background thread so the TUI
/// thread never performs blocking filesystem I/O.
struct RefreshTimers {
    last_filter_click: Instant,
    last_port_refresh: Instant,
    port_scan_initialized: bool,
}

/// All fields for the ⓘ notices button and its popup.
struct NoticesState {
    button_col: Option<u16>,
    missing_hook_groups: Vec<NoticesMissingHookGroup>,
    claude_plugin_installed_version: Option<String>,
    claude_settings_has_residual_hooks: bool,
    claude_plugin_notice: Option<ClaudePluginNotice>,
    copy_targets: Vec<NoticesCopyTarget>,
    copied_at: Option<(String, Instant)>,
}
```

---

## State Invariants

1. `selected_pane_row` is always < `layout.pane_row_targets.len()` — clamped in `rebuild_row_targets()`
2. `activity.entries` contains only the focused pane's entries — cleared on focus change
3. Tab preferences persist per pane in `PaneRuntimeState.tab_pref` and are restored on focus change. They vanish together with the rest of `PaneRuntimeState` when the pane is pruned, so a relaunched agent starts on the default tab
4. Git fetching respects the `git_tab_active` flag — stops when tab is hidden
5. Task progress has a 3-second debounce — prevents flicker when agent briefly pauses
6. Global state syncs via tmux variables — enables coordination across sidebar instances
7. Scroll positions are independent per panel — agents, activity, git each have their own `ScrollState`
8. `layout.line_to_row` is rebuilt every frame — ensures accurate click routing
9. Pane runtime state is pruned when the pane disappears — prevents stale per-pane ports, task progress, and tab preferences from surviving after the agent is gone
10. At most one popup is open at a time — enforced structurally by the `PopupState` enum, not by parallel boolean flags
10. Hook-based cleanup wins when available; pid-based cleanup is a slower fallback that removes panes when the agent process is gone but the hook did not fire
