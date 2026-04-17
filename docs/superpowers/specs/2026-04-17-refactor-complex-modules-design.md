# Refactor: Decompose Complex Modules — Design

**Date**: 2026-04-17
**Status**: Approved — pending implementation plan
**Author**: Hiroppy (with Claude)

## Summary

Decompose four high-complexity modules in tmux-agent-sidebar while preserving every observable behavior (byte-identical UI snapshots, unchanged external CLI/hook/tmux-option surface). Ship as a single PR, committed in 13 reviewable phases.

## Scope

### In Scope

1. **`src/state.rs`** (3471 lines) — Decompose `AppState` (32 fields) into domain-focused sub-structs and split the file into a `src/state/` module tree.
2. **`src/ui/panes.rs:487` `draw_agents()`** (~270 lines) — Split into 4–5 responsibility-focused functions and a `src/ui/panes/` submodule tree.
3. **`src/cli/hook.rs:277` `handle_event()`** (~165 lines) — Reduce destructuring duplication, split the file into a `src/cli/hook/` submodule tree.
4. **`src/tmux.rs`** — Extract `WorktreeMetadata` from `PaneInfo`; replace magic field indices in `parse_pane_line` with named constants. No file split.
5. **Test coverage** — Every extracted unit gets at least one unit test. Existing untested code is not in scope (keep this PR focused).

### Out of Scope

- External contracts: `hook` / `toggle` / `auto-close` / `set-status` CLI arguments, `/tmp/tmux-agent-*` file format, tmux-option keys (`@pane_*`, `@sidebar_*`).
- Feature additions, bug fixes, performance work.
- `Cargo.toml` dependency changes.
- Changes to `git.rs`, `activity.rs`, `group.rs`, `ui/bottom.rs`, adapters, etc. beyond the minimal call-site updates required by moved/renamed fields.
- `main.rs` event loop restructuring.
- Adding mock layers for `tmux`/`gh`/filesystem I/O.
- Changing `AgentEvent` enum definition in `src/event.rs`.
- CI coverage integration.

## Success Criteria

All must pass before merge:

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings` (no new warnings vs baseline)
- `cargo test` — all 912+ existing tests pass, plus new unit tests for extracted units
- `cargo build --release` succeeds
- `cargo insta pending-snapshots` reports zero entries (UI output is byte-identical)
- Function coverage on touched files does not regress (measured locally with `cargo-llvm-cov`)
- Extracted units have 100% function coverage (at least one test per public function)
- Manual smoke test in a live tmux session: status icons, tab switching, popups, filters, scrolling, and hook events all render correctly

## Target Module Structure

### `src/state/` (expanded from existing `state.rs` + `state/refresh.rs` + `state/tab.rs`)

```
src/state.rs                     — AppState struct + top-level impl, pub use re-exports
src/state/refresh.rs             — existing
src/state/tab.rs                 — existing
src/state/activity.rs            — ActivityState (new)
src/state/scroll.rs              — ScrollState + ScrollStates (new)
src/state/focus.rs               — Focus enum + FocusState (new)
src/state/layout.rs              — FrameLayout, RowTarget, RepoSpawnTarget,
                                   SpawnRemoveTarget, HyperlinkOverlay (moved)
src/state/popup.rs               — PopupState, SpawnField (moved)
src/state/notices.rs             — NoticesState, ClaudePluginNotice,
                                   NoticesMissingHookGroup, NoticesCopyTarget (moved)
src/state/timers.rs              — RefreshTimers (moved)
src/state/pane_runtime.rs        — PaneRuntimeState + PaneRuntimeMap wrapper (new)
src/state/filter.rs              — StatusFilter, RepoFilter (moved)
src/state/global.rs              — GlobalState (moved)
src/state/session.rs             — SessionNamesState (new)
```

The existing Rust convention of `state.rs` + `state/` sibling directory is preserved. Each submodule owns its type definitions and local `impl` blocks. `pub use` in `state.rs` keeps external names (`ScrollState`, `PopupState`, etc.) visible so unrelated callers are not forced to update `use` paths.

### `src/ui/panes/` (new subdirectory; `src/ui/panes.rs` becomes coordinator)

```
src/ui/panes.rs                  — draw_agents() coordinator (<100 lines target)
src/ui/panes/row.rs              — existing (pane row rendering)
src/ui/panes/filter_bar.rs       — render_filter_bar, render_secondary_header (new)
src/ui/panes/row_collector.rs    — CollectedRows + collect() (new)
src/ui/panes/click_targets.rs    — materialize() (new)
src/ui/panes/popups.rs           — render_if_open() (new)
```

### `src/cli/hook/` (new subdirectory; `src/cli/hook.rs` becomes dispatch only)

```
src/cli/hook.rs                  — entry + handle_event() dispatch (<100 lines target)
src/cli/hook/context.rs          — AgentContext, make_ctx, set_agent_meta,
                                   pane_writes_allowed, clear_all_meta,
                                   clear_run_state, mark_pending,
                                   drain_pending_teardowns, PENDING_* constants
src/cli/hook/handlers.rs         — on_* event handler functions
src/cli/hook/activity.rs         — handle_activity_log + related
src/cli/hook/notifications.rs    — desktop notification glue
```

### `src/tmux.rs` — no file split

`WorktreeMetadata` sub-struct added alongside `PaneInfo`. `parse_pane_line` gets named constants for field indices. Existing auxiliary functions (`resolve_codex_permission_modes`, `finalize_sessions`, `build_session_hierarchy`) remain as-is.

## `AppState` Field Mapping

Current 32 flat fields redistributed as follows:

```rust
pub struct AppState {
    // Unchanged top-level fields
    pub now: u64,
    pub tmux_pane: String,
    pub spinner_frame: usize,
    pub repo_groups: Vec<crate::group::RepoGroup>,
    pub theme: ColorTheme,
    pub icons: StatusIcons,
    pub bottom_panel_height: u16,
    pub flash: Option<(String, Instant)>,
    pub pending_osc52_copy: Option<String>,
    pub version_notice: Option<crate::version::UpdateNotice>,
    pub git: crate::git::GitData,
    pub bottom_tab: BottomTab,

    // Extracted sub-structs
    pub focus_state: FocusState,
    pub activity: ActivityState,
    pub scrolls: ScrollStates,
    pub pane_states: PaneRuntimeMap,
    pub layout: FrameLayout,
    pub popup: PopupState,
    pub notices: NoticesState,
    pub timers: RefreshTimers,
    pub global: GlobalState,
    pub sessions: SessionNamesState,
}

pub struct FocusState {
    pub sidebar_focused: bool,
    pub focus: Focus,
    pub focused_pane_id: Option<String>,
    pub prev_focused_pane_id: Option<String>,
}

pub struct ActivityState {
    pub entries: Vec<ActivityEntry>,
    pub scroll: ScrollState,
    pub max_entries: usize,
    pub log_cache: Option<(String, std::time::SystemTime)>,
}

pub struct ScrollStates {
    pub panes: ScrollState,
    pub git: ScrollState,
    // note: activity scroll lives in ActivityState for cohesion
}

pub struct PaneRuntimeMap {
    pub map: HashMap<String, PaneRuntimeState>,
    pub seen: std::collections::HashSet<String>,
}

pub struct SessionNamesState {
    pub names: HashMap<String, String>,
    pub dirty: bool,
}
```

**Design rules**:
- Fields that stand alone semantically (`theme`, `icons`, `flash`, `bottom_tab`, `git`, `bottom_panel_height`) remain at the top level — do not force them into artificial groups.
- Activity-related state is consolidated into `ActivityState` (including its scroll), not split between `scrolls` and `activity`.
- `PaneRuntimeMap` is a thin wrapper over `HashMap + HashSet` with delegating `get`/`entry`/`insert` methods; existing `AppState` helpers (`pane_state_mut`, etc.) stay in `state.rs` and delegate through.

### Call-site Update Pattern

```rust
// Before
state.activity_entries.push(entry);
state.activity_scroll.offset = 0;
state.seen_agent_panes.insert(id);
state.focused_pane_id = Some(id);
state.session_names_dirty = true;

// After
state.activity.entries.push(entry);
state.activity.scroll.offset = 0;
state.pane_states.seen.insert(id);
state.focus_state.focused_pane_id = Some(id);
state.sessions.dirty = true;
```

Call-site updates are mechanical — the compiler catches every miss.

## `draw_agents()` Split

### Coordinator signature (post-refactor)

```rust
pub fn draw_agents(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let layout = PaneLayout::compute(area);
    render_filter_bar_into(frame, state, layout.filter_area);
    render_secondary_header_into(frame, state, layout.secondary_area);

    let collected = row_collector::collect(state, layout.list_area.width);
    let scroll_offset = compute_scroll_offset(state, &collected, layout.list_area);
    render_pane_rows(frame, &collected, scroll_offset, layout.list_area);

    click_targets::materialize(state, &collected, scroll_offset, layout.list_area);
    popups::render_if_open(frame, state, area);
}

struct PaneLayout {
    filter_area: Rect,
    secondary_area: Rect,
    list_area: Rect,
}
```

### Submodule responsibilities

**`src/ui/panes/row_collector.rs`**

```rust
pub struct CollectedRows<'a> {
    pub lines: Vec<Line<'a>>,
    pub line_to_row: Vec<Option<usize>>,
    pub pending_spawn: Vec<(usize, String, String)>,
    pub pending_remove: Vec<(usize, u16, String)>,
}

pub fn collect(state: &AppState, width: u16) -> CollectedRows<'_> { ... }
```

Iterates `repo_groups`, applies `StatusFilter`/`RepoFilter`, inserts group separators, builds `header_row`/`branch_ports_row`/`diff_row` lines, and collects pending click targets.

**`src/ui/panes/click_targets.rs`**

```rust
pub fn materialize(
    state: &mut AppState,
    collected: &CollectedRows,
    scroll_offset: usize,
    list_area: Rect,
) { ... }
```

Converts `line_to_row` / `pending_spawn` / `pending_remove` into absolute-screen `Rect` entries on `state.layout`.

**`src/ui/panes/popups.rs`**

```rust
pub fn render_if_open(frame: &mut Frame, state: &mut AppState, area: Rect) {
    match &state.popup {
        PopupState::None => {}
        PopupState::Spawn(..) => render_spawn_popup(frame, state, area),
        PopupState::Remove(..) => render_remove_popup(frame, state, area),
        PopupState::Notices(..) => render_notices_popup(frame, state, area),
        // ...
    }
}
```

### Snapshot Guarantees

- Line construction order and blank-line insertion logic are preserved byte-for-byte.
- `pending_*` collection timing (inside the loop) is preserved.
- Rect/column arithmetic is moved but expressions are not altered.
- After each commit, `cargo test -- ui_snapshot` confirms zero diffs.

## `handle_event()` Decomposition

### Approach

1. **Introduce `make_ctx()` helper** in `src/cli/hook/context.rs`:

   ```rust
   pub(super) fn make_ctx<'a>(
       agent: &'a str,
       cwd: &'a str,
       permission_mode: &'a str,
       worktree: &'a Option<WorktreeInfo>,
       session_id: &'a Option<String>,
   ) -> AgentContext<'a> {
       AgentContext { agent, cwd, permission_mode, worktree, session_id }
   }
   ```

   Collapses each contextual match arm from ~15 lines to ~3.

2. **Split file** into `src/cli/hook/{context,handlers,activity,notifications}.rs` per the structure above.

3. **Result**: `handle_event()` shrinks from 165 lines to ~50 lines of pure dispatch.

```rust
fn handle_event(pane: &str, agent_name: &str, event: AgentEvent) -> i32 {
    use AgentEvent::*;
    match event {
        SessionStart { agent, cwd, permission_mode, worktree, session_id, .. } =>
            handlers::on_session_start(
                pane,
                &make_ctx(&agent, &cwd, &permission_mode, &worktree, &session_id),
            ),
        SessionEnd => handlers::on_session_end(pane),
        UserPromptSubmit { agent, cwd, permission_mode, prompt, worktree, session_id, .. } =>
            handlers::on_user_prompt_submit(
                pane,
                &make_ctx(&agent, &cwd, &permission_mode, &worktree, &session_id),
                &prompt,
            ),
        ActivityLog { tool_name, tool_input, tool_response } =>
            activity::handle_activity_log(pane, &tool_name, &tool_input, &tool_response),
        // ... every remaining variant: 1–2 lines each
    }
}
```

### What Is Not Changed

- `AgentEvent` enum definition in `src/event.rs` stays untouched.
- `notification_settings()` is not hoisted — remains called lazily within variants that need it, to avoid unnecessary I/O for variants that do not.
- No `AgentEvent::context()` method added — variant-specific extra fields (`prompt`, `last_message`, `error`, `wait_reason`) make a generic accessor awkward.

## `tmux.rs` Changes

### `WorktreeMetadata` extraction

```rust
#[derive(Debug, Clone, Default)]
pub struct WorktreeMetadata {
    pub name: String,
    pub branch: String,
}

pub struct PaneInfo {
    // ... other fields unchanged ...
    pub worktree: WorktreeMetadata,
    // ... other fields unchanged ...
}
```

Call-site updates: `pane.worktree_name` → `pane.worktree.name`, `pane.worktree_branch` → `pane.worktree.branch`. All mechanical.

### Named field constants

In `parse_pane_line`, replace `parts[15]`, `parts[16]`, etc. with named constants:

```rust
const PANE_FIELD_WINDOW_ID: usize = 1;
const PANE_FIELD_PANE_CWD: usize = 15;
const PANE_FIELD_PERMISSION_MODE: usize = 16;
// ...
```

Purely cosmetic readability improvement; no behavior change.

### What Is Not Changed

- No `PermissionState` sub-struct — `permission_mode` is a single field with no coupled siblings.
- `build_session_hierarchy()` is not split further — already ~40 lines with supporting functions extracted.
- Field count on `PaneInfo` drops by one (two collapsed into `worktree`), stays at ~18.

## Test Coverage Strategy

### Principle (agreed option A)

- Every extracted unit gets at least one unit test (≥1 per public function).
- Existing untested code is not part of this PR.
- No absolute coverage target; the binding constraint is "touched files' function coverage does not decrease."

### Targets

- `row_collector::collect()` — empty / single-group / multi-group / status-filter-applied / repo-filter-applied inputs
- `click_targets::materialize()` — verify Rect math under varying scroll offsets
- `PaneLayout::compute()` — area → sub-rect splits including edge cases (height 0, 1, 2)
- `popups::render_if_open()` — covered by existing snapshots; add gap-filling tests if needed
- Each of 11 hook handler functions (`on_session_start`, `on_session_end`, `on_stop`, etc.) — input event → expected tmux option writes / side effects
- `make_ctx()` — basic construction
- `tmux::parse_pane_line` — confirm named-constant replacement does not change parse behavior (regression test on representative fixtures)
- `PaneRuntimeMap` delegation methods — `get`/`entry`/`insert`/`seen.insert`

### Tooling

- Install locally: `cargo install cargo-llvm-cov` (not added to CI)
- Before refactor: `cargo llvm-cov --lcov --output-path /tmp/cov-before.lcov`
- After each commit (or at minimum before merge): compare function coverage on touched files

### Excluded from Tests

- Code paths that shell out to `tmux` / `ps` / `gh` or touch the real filesystem (no mock layer added in this PR).
- `main.rs` event loop.

## Verification Strategy

### Per-commit

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo insta pending-snapshots   # must report no pending entries
```

### End-of-branch (before merge)

```bash
cargo build --release
cargo llvm-cov --html   # confirm no regressions on touched files
```

### Manual smoke test

- Launch sidebar in a live tmux session.
- Exercise: status icons (Running / Waiting / Idle / Error), Activity/GitStatus tabs, spawn/remove popup, notices popup, status/repo filters, scrolling.
- Fire real Claude Code and Codex hook events (`session-start`, `stop`, `notification`, etc.).
- Run for ≥5 minutes to exercise periodic refresh loops without crash or layout corruption.

## Commit Sequence (single PR, 13 commits)

Each commit leaves `cargo test` passing. Commits are grouped so the PR can, if needed, be later split along these boundaries.

```
 1. refactor(state): extract ActivityState and SessionNamesState
 2. refactor(state): extract FocusState and ScrollStates
 3. refactor(state): extract PaneRuntimeMap wrapper
 4. refactor(state): move remaining types to state/ submodules
 5. refactor(tmux): extract WorktreeMetadata from PaneInfo
 6. refactor(tmux): name-constant pane line field indices
 7. refactor(ui/panes): extract PaneLayout and filter_bar module
 8. refactor(ui/panes): extract row_collector
 9. refactor(ui/panes): extract click_targets and popups modules
10. refactor(cli/hook): extract context and handlers modules
11. refactor(cli/hook): split activity and notifications modules
12. test: add unit tests for extracted units
13. docs: update CLAUDE.md architecture section (if needed)
```

**Grouping for future-splittability**: commits 1–4 (state) / 5–6 (tmux) / 7–9 (UI) / 10–11 (hook) / 12–13 (tests+docs) each form a self-contained group.

## Rollback Plan

- Each commit is `cargo test`-green. If a regression is discovered mid-review, `git reset --soft HEAD~N` walks back to any prior checkpoint.
- If snapshots diverge unintentionally, the offending commit is identified by bisecting the commit sequence and the snapshot diff is resolved before proceeding — snapshots are never accepted via `cargo insta accept` during this PR.

## Open Questions

None. All design decisions confirmed in brainstorming.

## Next Step

Invoke the `writing-plans` skill to produce an implementation plan keyed to these 13 commits.
