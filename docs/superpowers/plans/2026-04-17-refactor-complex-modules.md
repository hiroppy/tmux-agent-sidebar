# Refactor Complex Modules — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Decompose four high-complexity modules (`AppState` in `state.rs`, `draw_agents()` in `ui/panes.rs`, `handle_event()` in `cli/hook.rs`, `PaneInfo` in `tmux.rs`) while preserving every observable behavior. Ship as a single PR with 13 reviewable commits.

**Architecture:** Pure refactor — no behavior changes. Domain-focused sub-structs replace flat fields on `AppState`. Responsibility-focused submodules replace large coordinator functions. External surface (CLI args, `/tmp/` file format, tmux-option keys, `AgentEvent` enum, UI rendering) is untouched.

**Tech Stack:** Rust 2024 edition, Ratatui + Crossterm (TUI), insta (inline snapshot tests), indexmap, serde_json.

**Spec:** [`2026-04-17-refactor-complex-modules-design.md`](../specs/2026-04-17-refactor-complex-modules-design.md) — read this before starting any task.

---

## Shared Rules (Apply to Every Commit)

### Verification checklist per commit

Run at the end of every commit, before `git commit`:

```bash
cargo fmt --check                            # must pass
cargo clippy                                 # must pass (matches CI; pre-existing warnings ok)
cargo test                                   # all tests green (912+ + any new)
cargo insta pending-snapshots                # must output nothing (no pending snapshots)
```

If `cargo insta pending-snapshots` outputs anything, **do not proceed**. Investigate the diff. Byte-identical UI output is a hard constraint. Never run `cargo insta accept` during this PR.

Run once before the first commit and once before the final commit:

```bash
cargo build --release                        # must succeed
```

### Commit message style

Follow the existing repo convention (lowercase type prefix, imperative mood):

```
refactor(state): extract ActivityState and SessionNamesState

Moves activity_entries, activity_scroll, activity_max_entries,
activity_log_cache into ActivityState and session_names,
session_names_dirty into SessionNamesState. Call sites updated.
```

Do **not** add `Co-Authored-By` or tool-generated footers unless the user asks.

### Finding call sites

For each field rename, list call sites with:

```bash
rg -n '\bactivity_entries\b' src/ tests/
```

Update every match. The compiler is the backstop — if you miss one, `cargo build` fails with a clear error pointing at the line.

### When a test module moves

If a test at the bottom of a file (`#[cfg(test)] mod tests`) exercises only functions that are being moved to a new submodule, move the test module with them. If it mixes moved and unmoved functions, split: the portion exercising moved code migrates, the rest stays. Never duplicate tests.

### Branch

Work on a new branch:

```bash
git checkout -b refactor/decompose-complex-modules
```

All 13 commits land on this branch.

---

## Chunk 1: `AppState` Decomposition (Commits 1–4)

### Task 1.1: Extract `ActivityState` and `SessionNamesState`

**Goal:** Move activity-related and session-name fields off `AppState` into dedicated sub-structs.

**Files:**
- Create: `src/state/activity.rs`
- Create: `src/state/session.rs`
- Modify: `src/state.rs` (struct definition + `impl AppState::new` + any inline helpers)
- Modify: `src/state/refresh.rs` (uses these fields)
- Modify: `src/main.rs`
- Modify: `src/ui/bottom/activity.rs`

- [ ] **Step 1: Create `src/state/activity.rs`**

```rust
use std::time::SystemTime;

use crate::activity::ActivityEntry;
use crate::state::ScrollState;

#[derive(Debug, Clone, Default)]
pub struct ActivityState {
    pub entries: Vec<ActivityEntry>,
    pub scroll: ScrollState,
    pub max_entries: usize,
    /// `(focused_pane_id, mtime)` of the activity log most recently
    /// rendered into `entries`. `refresh_activity_log` skips re-reading
    /// the log when neither field has changed.
    pub log_cache: Option<(String, SystemTime)>,
}

impl ActivityState {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            scroll: ScrollState::default(),
            max_entries: 50,
            log_cache: None,
        }
    }
}
```

- [ ] **Step 2: Create `src/state/session.rs`**

```rust
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct SessionNamesState {
    /// Maps session_id → session name, refreshed periodically from
    /// `~/.claude/sessions/*.json` files.
    pub names: HashMap<String, String>,
    /// `true` when `names` has changed since the last
    /// `refresh_session_names` application. Avoids re-walking every
    /// pane each tick when the map is unchanged (the polling thread
    /// only updates it every 10s).
    pub dirty: bool,
}

impl SessionNamesState {
    pub fn new() -> Self {
        Self {
            names: HashMap::new(),
            dirty: true,
        }
    }
}
```

- [ ] **Step 3: Declare submodules and re-export in `src/state.rs`**

Add after existing `mod refresh; mod tab;` lines (`src/state.rs:9-10`):

```rust
mod activity;
mod session;

pub use activity::ActivityState;
pub use session::SessionNamesState;
```

- [ ] **Step 4: Remove old fields from `AppState` and add new sub-struct fields**

In `src/state.rs:513` (struct `AppState`):

Remove:
- `pub activity_entries: Vec<ActivityEntry>,`
- `pub activity_scroll: ScrollState,`
- `pub activity_max_entries: usize,`
- `pub activity_log_cache: Option<(String, std::time::SystemTime)>,`
- `pub session_names: HashMap<String, String>,`
- `pub session_names_dirty: bool,`

Add:
- `pub activity: ActivityState,`
- `pub sessions: SessionNamesState,`

- [ ] **Step 5: Update `AppState::new()` in `src/state.rs`**

Replace the six initializers with:

```rust
activity: ActivityState::new(),
sessions: SessionNamesState::new(),
```

- [ ] **Step 6: Rewrite call sites — run `cargo build` and follow compiler errors**

Expected error pattern: `error[E0609]: no field ` followed by the old name.

Substitution table:

| Before | After |
|---|---|
| `state.activity_entries` | `state.activity.entries` |
| `state.activity_scroll` | `state.activity.scroll` |
| `state.activity_max_entries` | `state.activity.max_entries` |
| `state.activity_log_cache` | `state.activity.log_cache` |
| `state.session_names` | `state.sessions.names` |
| `state.session_names_dirty` | `state.sessions.dirty` |
| `self.activity_entries` | `self.activity.entries` (inside `impl AppState`) |
| `self.activity_scroll` | `self.activity.scroll` |
| `self.activity_max_entries` | `self.activity.max_entries` |
| `self.activity_log_cache` | `self.activity.log_cache` |
| `self.session_names` | `self.sessions.names` |
| `self.session_names_dirty` | `self.sessions.dirty` |

Grep to locate the call sites once, update each file, then loop `cargo build` until it compiles:

```bash
rg -n '\b(activity_entries|activity_scroll|activity_max_entries|activity_log_cache|session_names|session_names_dirty)\b' src/ tests/
```

- [ ] **Step 7: Run verification checklist**

```bash
cargo fmt
cargo fmt --check
cargo clippy
cargo test
cargo insta pending-snapshots
```

Expected: all pass, `pending-snapshots` output is empty.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor(state): extract ActivityState and SessionNamesState"
```

---

### Task 1.2: Extract `FocusState` and `ScrollStates`

**Goal:** Group focus-related fields into `FocusState` and non-activity scrolls into `ScrollStates`.

**Files:**
- Create: `src/state/focus.rs`
- Create: `src/state/scroll.rs`
- Modify: `src/state.rs`
- Modify: `src/state/refresh.rs`, `src/state/tab.rs`
- Modify: `src/main.rs`, `src/ui/panes.rs`, `src/ui/bottom.rs`, `src/ui/bottom/git.rs`

- [ ] **Step 1: Create `src/state/focus.rs`**

`Focus` enum currently lives at `src/state.rs:15`. Move it here, then add `FocusState`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Focus {
    Filter,
    Panes,
    ActivityLog,
}

#[derive(Debug, Clone, Default)]
pub struct FocusState {
    pub sidebar_focused: bool,
    pub focus: Focus,
    pub focused_pane_id: Option<String>,
    pub prev_focused_pane_id: Option<String>,
}

impl Default for Focus {
    fn default() -> Self {
        Self::Panes
    }
}

impl FocusState {
    pub fn new() -> Self {
        Self {
            sidebar_focused: false,
            focus: Focus::Panes,
            focused_pane_id: None,
            prev_focused_pane_id: None,
        }
    }
}
```

- [ ] **Step 2: Create `src/state/scroll.rs`**

`ScrollState` currently lives at `src/state.rs:275` with its `impl` block starting at `:281`. Move both here:

```rust
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    pub offset: usize,
    pub total_lines: usize,
    pub visible_height: usize,
}

impl ScrollState {
    // ... move the existing impl block verbatim from state.rs:281-309 ...
}

#[derive(Debug, Clone, Default)]
pub struct ScrollStates {
    pub panes: ScrollState,
    pub git: ScrollState,
}
```

Note: activity scroll lives in `ActivityState.scroll`, not here. This struct holds only `panes` and `git`.

- [ ] **Step 3: Declare submodules and re-export in `src/state.rs`**

Add after Task 1.1's additions:

```rust
mod focus;
mod scroll;

pub use focus::{Focus, FocusState};
pub use scroll::{ScrollState, ScrollStates};
```

Remove the local `Focus` and `ScrollState` definitions and their `impl` blocks from `state.rs` (they're now in the submodules).

- [ ] **Step 4: Update `AppState` struct and `new()`**

Remove from struct:
- `pub sidebar_focused: bool,`
- `pub focus: Focus,`
- `pub focused_pane_id: Option<String>,`
- `pub prev_focused_pane_id: Option<String>,`
- `pub panes_scroll: ScrollState,`
- `pub git_scroll: ScrollState,`

Add:
- `pub focus_state: FocusState,`
- `pub scrolls: ScrollStates,`

In `new()`:

```rust
focus_state: FocusState::new(),
scrolls: ScrollStates::default(),
```

- [ ] **Step 5: Rewrite call sites**

| Before | After |
|---|---|
| `state.sidebar_focused` | `state.focus_state.sidebar_focused` |
| `state.focus` | `state.focus_state.focus` |
| `state.focused_pane_id` | `state.focus_state.focused_pane_id` |
| `state.prev_focused_pane_id` | `state.focus_state.prev_focused_pane_id` |
| `state.panes_scroll` | `state.scrolls.panes` |
| `state.git_scroll` | `state.scrolls.git` |
| Same substitutions for `self.*` inside `impl AppState` blocks. |  |

Grep (run **once before starting edits** — running mid-edit will match `state.focus_state` as a false positive because `\bstate\.focus\b` prefix-matches the new name):

```bash
rg -n '\b(sidebar_focused|focused_pane_id|prev_focused_pane_id|panes_scroll|git_scroll)\b' src/ tests/
rg -n '\bstate\.focus\b|\bself\.focus\b' src/
```

For `state.focus` / `self.focus`, be careful: match is on `state.focus` as a whole, not `state.focus_state.*`. After substitution, `state.focus_state.focus` should read naturally.

Known call site not caught by the first grep: `src/ui/bottom.rs:30` reads `state.focus == Focus::ActivityLog`. Rewrite to `state.focus_state.focus == Focus::ActivityLog`.

- [ ] **Step 6: Run verification checklist + commit**

```bash
cargo fmt && cargo fmt --check && cargo clippy && cargo test && cargo insta pending-snapshots
git add -A
git commit -m "refactor(state): extract FocusState and ScrollStates"
```

---

### Task 1.3: Extract `PaneRuntimeMap` wrapper

**Goal:** Bundle `pane_states` (HashMap) and `seen_agent_panes` (HashSet) into a `PaneRuntimeMap` wrapper.

**Files:**
- Create: `src/state/pane_runtime.rs`
- Modify: `src/state.rs`
- Modify: `src/state/refresh.rs`, `src/state/tab.rs`

- [ ] **Step 1: Create `src/state/pane_runtime.rs`**

Move `PaneRuntimeState` (currently at `src/state.rs:128`) here, then add the map wrapper:

```rust
use std::collections::{HashMap, HashSet};

use crate::activity::TaskProgress;
use crate::state::BottomTab;

#[derive(Debug, Clone, Default)]
pub struct PaneRuntimeState {
    pub ports: Vec<u16>,
    pub command: Option<String>,
    pub task_progress: Option<TaskProgress>,
    pub task_dismissed_total: Option<usize>,
    pub inactive_since: Option<u64>,
    pub tab_pref: Option<BottomTab>,
    pub task_progress_log_mtime: Option<std::time::SystemTime>,
}

#[derive(Debug, Clone, Default)]
pub struct PaneRuntimeMap {
    pub map: HashMap<String, PaneRuntimeState>,
    /// Agent pane IDs that have already been seen.
    pub seen: HashSet<String>,
}

impl PaneRuntimeMap {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            seen: HashSet::new(),
        }
    }

    pub fn get(&self, pane_id: &str) -> Option<&PaneRuntimeState> {
        self.map.get(pane_id)
    }

    pub fn get_mut(&mut self, pane_id: &str) -> Option<&mut PaneRuntimeState> {
        self.map.get_mut(pane_id)
    }

    pub fn entry_mut(&mut self, pane_id: &str) -> &mut PaneRuntimeState {
        self.map.entry(pane_id.to_string()).or_default()
    }

    pub fn contains_key(&self, pane_id: &str) -> bool {
        self.map.contains_key(pane_id)
    }

    pub fn remove(&mut self, pane_id: &str) -> Option<PaneRuntimeState> {
        self.map.remove(pane_id)
    }
}
```

- [ ] **Step 2: Declare submodule and re-export**

```rust
mod pane_runtime;

pub use pane_runtime::{PaneRuntimeMap, PaneRuntimeState};
```

Remove the local `PaneRuntimeState` definition from `state.rs`.

- [ ] **Step 3: Update `AppState` struct and `new()`**

Remove:
- `pub pane_states: HashMap<String, PaneRuntimeState>,`
- `pub seen_agent_panes: std::collections::HashSet<String>,`

Add:
- `pub pane_states: PaneRuntimeMap,`

In `new()`:

```rust
pane_states: PaneRuntimeMap::new(),
```

- [ ] **Step 4: Rewrite call sites**

The existing `impl AppState::pane_state_mut` / `pane_state` methods (at `src/state.rs:664-669`) should stay, but delegate:

```rust
pub fn pane_state_mut(&mut self, pane_id: &str) -> &mut PaneRuntimeState {
    self.pane_states.entry_mut(pane_id)
}

pub fn pane_state(&self, pane_id: &str) -> Option<&PaneRuntimeState> {
    self.pane_states.get(pane_id)
}
```

Substitutions:

| Before | After |
|---|---|
| `state.pane_states.get(...)` / `self.pane_states.get(...)` | unchanged (delegation method matches) |
| `state.pane_states.get_mut(...)` / `self.pane_states.get_mut(...)` | unchanged (delegation method matches) |
| `state.pane_states.insert(...)` | `state.pane_states.map.insert(...)` |
| `state.pane_states.remove(...)` / `self.pane_states.remove(...)` | unchanged (delegated) |
| `state.pane_states.iter()` / `.iter_mut()` / `.values()` / `.values_mut()` / `.keys()` | `state.pane_states.map.<same>` |
| `state.pane_states.retain(...)` / `self.pane_states.retain(...)` | `state.pane_states.map.retain(...)` / `self.pane_states.map.retain(...)` |
| `state.pane_states.entry(...)` | `state.pane_states.map.entry(...)` (or use `entry_mut()` helper) |
| `state.seen_agent_panes` | `state.pane_states.seen` |

Rule of thumb: if the call treated `pane_states` like a `HashMap`, prefix with `.map`. The delegating methods (`get`, `get_mut`, `remove`, `contains_key`, `entry_mut`) cover the common lookups verbatim.

Known call sites to watch:
- `src/state.rs:1199-1200` — `self.pane_states.retain(...)` → `self.pane_states.map.retain(...)` (inside `prune_pane_states_to_current_panes`)
- `src/state/tab.rs:82` — `self.pane_states.get_mut(id)` → unchanged (delegation method matches)

```bash
rg -n '\b(pane_states|seen_agent_panes)\b' src/ tests/
```

- [ ] **Step 5: Run verification checklist + commit**

```bash
cargo fmt && cargo fmt --check && cargo clippy && cargo test && cargo insta pending-snapshots
git add -A
git commit -m "refactor(state): extract PaneRuntimeMap wrapper"
```

---

### Task 1.4: Move remaining types to `state/` submodules

**Goal:** Consolidate the rest of `state.rs` type definitions into topical submodules; `state.rs` keeps only `AppState` and its top-level `impl`.

**Files:**
- Create: `src/state/layout.rs` — `FrameLayout`, `RowTarget`, `RepoSpawnTarget`, `SpawnRemoveTarget`, `HyperlinkOverlay`
- Create: `src/state/popup.rs` — `PopupState`, `SpawnField`
- Create: `src/state/notices.rs` — `NoticesState`, `ClaudePluginNotice`, `NoticesMissingHookGroup`, `NoticesCopyTarget`
- Create: `src/state/timers.rs` — `RefreshTimers`
- Create: `src/state/filter.rs` — `StatusFilter`, `RepoFilter`
- Create: `src/state/global.rs` — `GlobalState`
- Modify: `src/state.rs`

- [ ] **Step 1: Create each submodule file and move the type definitions**

For every type listed above:
1. Cut the `pub struct` / `pub enum` and its `impl` block(s) from `state.rs`.
2. Paste into the corresponding new file.
3. Add any `use` statements the moved code needs (compiler errors will tell you which).

Use `pub use` in `state.rs` so existing external imports (`use crate::state::PopupState;`) keep working:

```rust
mod layout;
mod popup;
mod notices;
mod timers;
mod filter;
mod global;

pub use layout::{FrameLayout, RowTarget, RepoSpawnTarget, SpawnRemoveTarget, HyperlinkOverlay};
pub use popup::{PopupState, SpawnField};
pub use notices::{NoticesState, ClaudePluginNotice, NoticesMissingHookGroup, NoticesCopyTarget};
pub use timers::RefreshTimers;
pub use filter::{StatusFilter, RepoFilter};
pub use global::GlobalState;
```

- [ ] **Step 2: Move inline tests if any**

`state.rs` has a `#[cfg(test)] mod tests` at the bottom. Scan the tests: those that exercise `FrameLayout`, `GlobalState`, `StatusFilter`, etc. move into the corresponding submodule's own `#[cfg(test)] mod tests`. Cross-cutting tests (that exercise `AppState` as a whole) stay in `state.rs`.

Rule: a test goes where its unit-under-test lives. Never duplicate.

- [ ] **Step 3: Run verification checklist + commit**

```bash
cargo fmt && cargo fmt --check && cargo clippy && cargo test && cargo insta pending-snapshots
git add -A
git commit -m "refactor(state): move type definitions into state/ submodules"
```

After this task, `src/state.rs` should be well under 1000 lines — holding `AppState`, its `new()`, and cross-cutting impl methods (`selected_pane`, `pane_state_mut`, etc.), plus the `pub use` re-exports.

---

## Chunk 2: `tmux.rs` Cleanup (Commits 5–6)

### Task 2.1: Extract `WorktreeMetadata` from `PaneInfo`

**Goal:** Collapse `worktree_name` + `worktree_branch` on `PaneInfo` into a single `WorktreeMetadata` sub-struct.

**Files:**
- Modify: `src/tmux.rs`
- Modify: `src/state/refresh.rs`, `src/state/tab.rs`, `src/group.rs`, `src/ui/panes.rs`, `src/ui/panes/row.rs`, `src/ui/text.rs`
- Modify: `tests/ui_snapshot.rs`, `tests/test_helpers.rs`, `tests/state_tests.rs`, `tests/color_tests.rs`, `tests/bottom_tests.rs`

- [ ] **Step 1: Add `WorktreeMetadata` and update `PaneInfo` in `src/tmux.rs`**

Right after the `PaneInfo` declaration (currently at `src/tmux.rs:7-32`):

```rust
#[derive(Debug, Clone, Default)]
pub struct WorktreeMetadata {
    pub name: String,
    pub branch: String,
}
```

Modify `PaneInfo`:

```rust
pub struct PaneInfo {
    // ... fields unchanged ...
    // Replace these two:
    //   pub worktree_name: String,
    //   pub worktree_branch: String,
    // With:
    pub worktree: WorktreeMetadata,
    // ... remaining fields unchanged ...
}
```

- [ ] **Step 2: Update construction sites**

In `src/tmux.rs` inside `parse_pane_line`, find where `worktree_name` and `worktree_branch` are assigned to the `PaneInfo` literal. Replace with:

```rust
worktree: WorktreeMetadata {
    name: /* existing name expression */,
    branch: /* existing branch expression */,
},
```

The tests in `tests/test_helpers.rs` construct `PaneInfo` literals — each needs the same change.

- [ ] **Step 3: Rewrite call sites**

| Before | After |
|---|---|
| `pane.worktree_name` | `pane.worktree.name` |
| `pane.worktree_branch` | `pane.worktree.branch` |

```bash
rg -n '\b(worktree_name|worktree_branch)\b' src/ tests/
```

- [ ] **Step 4: Run verification checklist + commit**

```bash
cargo fmt && cargo fmt --check && cargo clippy && cargo test && cargo insta pending-snapshots
git add -A
git commit -m "refactor(tmux): extract WorktreeMetadata from PaneInfo"
```

---

### Task 2.2: Replace `parse_pane_line` magic field indices with named constants

**Goal:** Improve readability of the tmux-field parser without changing behavior.

**Files:**
- Modify: `src/tmux.rs`

- [ ] **Step 1: Enumerate current indices**

Read `src/tmux.rs:303-400` (function `parse_pane_line`). Note every `parts[N]` access. Currently these use indices: 0, 3, 5, 6, 7, 9, 10, 13, 15, 16, 19, and more.

Cross-reference against `build_session_hierarchy` which consumes `parts[0..6]` before passing the rest (`parts[6..]`) to `parse_pane_line`. Constants must reflect the **absolute** format string order so both functions can share them.

- [ ] **Step 2: Add constants near the top of `tmux.rs`**

After the existing `pub const CLAUDE_AGENT` / `CODEX_AGENT` constants (line 4-5), add:

```rust
// Field indices in the `tmux list-panes -F` format string. Update together
// with the format string if the format changes. Indices are absolute
// (from the start of the full output line); functions that receive a
// subset (e.g. `parse_pane_line` after slicing past the window fields)
// adjust their own offsets.
mod pane_field {
    pub const SESSION_NAME: usize = 0;
    pub const WINDOW_ID: usize = 1;
    // ... enumerate all indices used ...
}
```

Use a `mod` namespace so constants are grouped but callers write `pane_field::WINDOW_ID` for clarity.

- [ ] **Step 3: Replace `parts[N]` usages**

For each numeric literal index in `parse_pane_line` and `build_session_hierarchy`, replace with the named constant. Example:

```rust
// Before:
let pane_cwd = &parts[15];

// After:
let pane_cwd = &parts[pane_field::PANE_CWD];
```

- [ ] **Step 4: Run verification checklist + commit**

```bash
cargo fmt && cargo fmt --check && cargo clippy && cargo test && cargo insta pending-snapshots
git add -A
git commit -m "refactor(tmux): name-constant pane line field indices"
```

---

## Chunk 3: `draw_agents()` Split (Commits 7–9)

### Task 3.1: Extract `PaneLayout` and move filter-bar helpers

**Goal:** Move `render_filter_bar` and `render_secondary_header` into `src/ui/panes/filter_bar.rs` and add `PaneLayout` as a coordinator helper.

**Files:**
- Create: `src/ui/panes/filter_bar.rs`
- Modify: `src/ui/panes.rs`

- [ ] **Step 1: Locate source functions**

- `render_filter_bar` at `src/ui/panes.rs:53` (ends at line ~116 — read to confirm)
- `render_secondary_header` at `src/ui/panes.rs:118` (ends at line ~200 — read to confirm)
- Associated tests at `src/ui/panes.rs:763+` — identify test functions that call only these two (from the earlier grep: `render_secondary_header_keeps_repo_position_*`, `render_filter_bar_is_status_only`, `render_filter_bar_uses_selected_and_inactive_icon_colors`, `render_secondary_header_repo_button_col_returned`, `render_secondary_header_shows_repo_name_when_filtered`, `render_secondary_header_truncates_long_repo_name`, `render_secondary_header_popup_open_styling`)

- [ ] **Step 2: Create `src/ui/panes/filter_bar.rs`**

Move the two functions verbatim. Change their visibility to `pub(super)`. Import whatever the compiler asks for.

At the bottom of the new file, add `#[cfg(test)] mod tests` containing the migrated test functions. Adjust `use` imports (tests that used `super::*` now need `use super::*;` pointing at `filter_bar`'s module).

- [ ] **Step 3: Add `PaneLayout` coordinator helper in `src/ui/panes.rs`**

Near the top of `panes.rs`, below imports:

```rust
struct PaneLayout {
    filter_area: Rect,
    secondary_area: Rect,
    list_area: Rect,
}

impl PaneLayout {
    fn compute(area: Rect) -> Self {
        let filter_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1.min(area.height),
        };
        let secondary_area = Rect {
            x: area.x,
            y: area.y + 1,
            width: area.width,
            height: 1.min(area.height.saturating_sub(1)),
        };
        let list_area = Rect {
            x: area.x,
            y: area.y + 2,
            width: area.width,
            height: area.height.saturating_sub(2),
        };
        Self { filter_area, secondary_area, list_area }
    }
}
```

Update `draw_agents` (line 487+) to use `PaneLayout::compute(area)` and reference `layout.filter_area`, `layout.secondary_area`, `layout.list_area` instead of the three local `Rect {...}` constructions. Preserve the order and the surrounding code exactly.

- [ ] **Step 4: Declare submodule and update imports**

In `src/ui/panes.rs`, add near the top:

```rust
mod filter_bar;
```

Inside `draw_agents`, change `render_filter_bar(...)` → `filter_bar::render_filter_bar(...)` and `render_secondary_header(...)` → `filter_bar::render_secondary_header(...)`.

If the existing `row.rs` file already uses `mod row;` somewhere, follow the same pattern for consistency.

- [ ] **Step 5: Wrap secondary-header writes in a `render_secondary_header_into` helper**

Immediately before `draw_agents`, add:

```rust
fn render_secondary_header_into(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let (line, notices_btn_col, repo_btn_col) =
        filter_bar::render_secondary_header(state, area.width);
    state.notices.button_col = notices_btn_col;
    state.layout.repo_button_col = repo_btn_col;
    frame.render_widget(Paragraph::new(vec![line]), area);
}
```

Replace the corresponding inline block in `draw_agents` (pre-refactor lines 501–511) with a single call:

```rust
render_secondary_header_into(frame, state, layout.secondary_area);
```

Same idea for `render_filter_bar_into`:

```rust
fn render_filter_bar_into(frame: &mut Frame, state: &AppState, area: Rect) {
    let line = filter_bar::render_filter_bar(state, area.width);
    frame.render_widget(Paragraph::new(vec![line]), area);
}
```

And use it at the top of `draw_agents`.

- [ ] **Step 6: Run verification checklist**

Critical: `cargo insta pending-snapshots` must be empty. If snapshots diverge, stop and find the behavioral regression. Common causes: wrong evaluation order of side effects (`state.notices.button_col` / `state.layout.repo_button_col`), or accidental change to the `Rect` calculation.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor(ui/panes): extract PaneLayout and filter_bar module"
```

---

### Task 3.2: Extract `row_collector`

**Goal:** Pull the repo-group iteration, filter application, line-building, and `pending_spawn`/`pending_remove` collection out of `draw_agents` into `src/ui/panes/row_collector.rs`.

**Files:**
- Create: `src/ui/panes/row_collector.rs`
- Modify: `src/ui/panes.rs`

- [ ] **Step 1: Identify the block to extract**

In `src/ui/panes.rs` `draw_agents`, the block to move is the repo-groups loop that builds `lines`, `line_to_row`, `pending_spawn_targets`, and `pending_remove_targets`. Line numbers shifted during Task 3.1, so use stable anchors to locate it:

```bash
# First line of the block (declaration of the local mutables):
rg -n 'let mut pending_spawn_targets' src/ui/panes.rs

# Last line of the block (just before scroll-offset computation begins):
rg -n 'pending_remove_targets' src/ui/panes.rs
```

The block runs from the `let mut lines: Vec<Line<'_>> = Vec::new();` declaration through the closing brace of the `for group in &state.repo_groups` loop.

- [ ] **Step 2: Create `src/ui/panes/row_collector.rs`**

```rust
use ratatui::text::Line;

use crate::state::AppState;

#[derive(Debug, Default)]
pub(super) struct CollectedRows {
    pub lines: Vec<Line<'static>>,
    pub line_to_row: Vec<Option<usize>>,
    pub pending_spawn: Vec<(usize, String, String)>,
    pub pending_remove: Vec<(usize, u16, String)>,
}

pub(super) fn collect(state: &AppState, width: u16) -> CollectedRows {
    // Move the contents of the pre-refactor block here verbatim.
    // Replace the local mutable vectors with fields on CollectedRows,
    // returning it at the end. Do not alter iteration order, filter
    // checks, line construction, or pending_* insertion points.
    //
    // For any Line<'_> with a non-'static lifetime, convert via
    // `.into_owned()` or by building the Spans from owned Strings.
    // All existing row builders already return Line<'static> or build
    // from owned data, so this should be a straight move.
    todo!()
}
```

Fill in `todo!()` by moving the block. If any `Line<'_>` borrows from `state`, convert to owned. Verify via `cargo build`.

- [ ] **Step 3: Update `draw_agents` to use `row_collector::collect`**

```rust
let collected = row_collector::collect(state, layout.list_area.width);
```

Then rename the downstream variable uses: `lines` → `collected.lines`, `line_to_row` → `collected.line_to_row`, `pending_spawn_targets` → `collected.pending_spawn`, `pending_remove_targets` → `collected.pending_remove`.

- [ ] **Step 4: Declare submodule**

```rust
mod row_collector;
```

- [ ] **Step 5: Verification checklist**

Snapshots must remain byte-identical. `cargo insta pending-snapshots` must be empty.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor(ui/panes): extract row_collector"
```

---

### Task 3.3: Extract `click_targets` and `popups` modules

**Goal:** Move the post-layout click-target materialization and popup rendering out of `draw_agents`.

**Files:**
- Create: `src/ui/panes/click_targets.rs`
- Create: `src/ui/panes/popups.rs`
- Modify: `src/ui/panes.rs`

- [ ] **Step 1: Identify the two blocks**

In the current `draw_agents` (post Task 3.2), locate the blocks by stable anchors:

```bash
# Click-targets block — converts collected line/pending vectors into
# absolute Rect entries on state.layout. Starts with a write to
# state.layout.pane_row_targets and ends just before the popup dispatch.
rg -n 'state\.layout\.pane_row_targets' src/ui/panes.rs

# Popups block — guarded dispatch to the render_*_popup helpers.
rg -n 'is_notices_popup_open|is_repo_popup_open|is_spawn_input_open|is_remove_confirm_open' src/ui/panes.rs
```

The click-targets block runs from the first `state.layout.pane_row_targets` assignment through the last pending-target conversion. The popups block is the chain of `if state.is_*_open() { render_*_popup(...); }` calls — move them all.

- [ ] **Step 2: Create `src/ui/panes/click_targets.rs`**

```rust
use ratatui::layout::Rect;

use crate::state::AppState;
use super::row_collector::CollectedRows;

pub(super) fn materialize(
    state: &mut AppState,
    collected: &CollectedRows,
    scroll_offset: usize,
    list_area: Rect,
) {
    // Move the click-targets block here verbatim. Preserve the order
    // in which `state.layout.*` fields are written — it matters for
    // mouse-routing tests.
    todo!()
}
```

- [ ] **Step 3: Create `src/ui/panes/popups.rs`**

```rust
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::state::AppState;

pub(super) fn render_if_open(frame: &mut Frame, state: &mut AppState, area: Rect) {
    // Move the popup dispatch block verbatim. Use the existing
    // is_*_open() helpers — do not replace with `match &state.popup`.
    todo!()
}
```

- [ ] **Step 4: Extract `compute_scroll_offset` as well**

```rust
fn compute_scroll_offset(
    state: &mut AppState,
    collected: &CollectedRows,
    list_area: Rect,
) -> usize {
    // Move the scroll-offset clamping logic (pre-refactor lines 651–678)
    // here verbatim. Mutates state.scrolls.panes.{total_lines,
    // visible_height, offset} exactly as before.
    todo!()
}
```

This stays in `src/ui/panes.rs` (no submodule needed — it's small and only `draw_agents` calls it).

Also extract `render_pane_rows`:

```rust
fn render_pane_rows(frame: &mut Frame, collected: &CollectedRows, scroll_offset: usize, area: Rect) {
    // Move the slicing + frame.render_widget call that draws the
    // visible slice of collected.lines from pre-refactor lines
    // immediately after the scroll-offset computation.
    todo!()
}
```

- [ ] **Step 5: Rewrite `draw_agents` as the thin coordinator**

Target shape:

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
```

- [ ] **Step 6: Declare new submodules**

```rust
mod click_targets;
mod popups;
```

- [ ] **Step 7: Verification checklist**

The UI snapshot suite (`tests/ui_snapshot.rs`, 72 tests) is the decisive check. Zero pending snapshots. If any diverge, bisect: revert to pre-Task-3.3 HEAD and re-apply each extracted block one at a time.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor(ui/panes): extract click_targets and popups modules"
```

---

## Chunk 4: `handle_event()` Split (Commits 10–11)

### Task 4.1: Extract `context` and `handlers` modules

**Goal:** Move `AgentContext`, `make_ctx`, shared helpers, and the 11 `on_*` handler functions into submodules. Reduce `handle_event` dispatch boilerplate.

**Files:**
- Create: `src/cli/hook/context.rs`
- Create: `src/cli/hook/handlers.rs`
- Modify: `src/cli/hook.rs`

- [ ] **Step 1: Locate the pieces**

In `src/cli/hook.rs`:
- `AgentContext<'a>` at line 76
- Shared helpers: `pane_writes_allowed` (:87), `set_agent_meta` (:92), `clear_run_state` (:104), `is_system_message` (:110), `clear_all_meta` (:114), `PENDING_*` constants (:136-138), `mark_pending` (:140), `drain_pending_teardowns` (:148), `run_session_end_teardown` (:165+)
- The `on_*` handler functions scattered between lines ~200 and ~900 (grep `fn on_session_start`, `fn on_session_end`, etc.)
- Existing tests in `#[cfg(test)] mod tests` at :948+

- [ ] **Step 2: Create `src/cli/hook/context.rs`**

Move `AgentContext<'a>` and all shared helpers into this file. Change visibility to `pub(super)`. Add a new `make_ctx` helper:

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

Move tests that exercise only helpers in this file (e.g. `pane_writes_allowed`, `set_agent_meta`, `clear_run_state`, `clear_all_meta`) into an `#[cfg(test)] mod tests` at the bottom of `context.rs`.

- [ ] **Step 3: Create `src/cli/hook/handlers.rs`**

Move all `on_*` functions here. Change their visibility to `pub(super)`. Add `use super::context::*;` to access shared helpers.

Move tests exercising only `on_*` functions (e.g. `on_session_start_*`, `on_stop_*`, `on_worktree_remove_*`) into `#[cfg(test)] mod tests` at the bottom of `handlers.rs`.

- [ ] **Step 4: Declare submodules and update `handle_event`**

In `src/cli/hook.rs`:

```rust
mod context;
mod handlers;

use context::make_ctx;
```

Rewrite `handle_event` (:277) to use `make_ctx` for each contextual variant:

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
        Notification { agent, cwd, permission_mode, wait_reason, meta_only, worktree, session_id, .. } => {
            let notifications = notification_settings();
            handlers::on_notification(
                pane,
                &make_ctx(&agent, &cwd, &permission_mode, &worktree, &session_id),
                &wait_reason,
                meta_only,
                &notifications,
            )
        }
        // ... continue for each remaining variant, following the same pattern ...
    }
}
```

- [ ] **Step 5: Verification checklist**

The hook tests are the primary check. Watch for `error[E0603]: module ... is private` — indicates a visibility mistake (use `pub(super)`, not `pub(crate)` or private).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor(cli/hook): extract context and handlers modules"
```

---

### Task 4.2: Split `activity` and `notifications` submodules

**Goal:** Move `handle_activity_log` and notification-related helpers out of `hook.rs` to shrink the dispatch file further.

**Files:**
- Create: `src/cli/hook/activity.rs`
- Create: `src/cli/hook/notifications.rs`
- Modify: `src/cli/hook.rs`

- [ ] **Step 1: Move `handle_activity_log`**

Currently referenced in `handle_event` as `handle_activity_log(pane, &tool_name, &tool_input, &tool_response)`. Find the function definition (grep `fn handle_activity_log` in `hook.rs`) and any helpers it calls that aren't shared with other handlers. Move to `src/cli/hook/activity.rs` with `pub(super)`. Migrate associated tests.

- [ ] **Step 2: Move `notification_settings` and notification helpers**

Grep `fn notification_settings`, `fn notification_run_id`, and related. Move to `src/cli/hook/notifications.rs` with `pub(super)`. The key question is: who else calls these? If `handlers.rs` calls `notification_settings`, `handlers.rs` needs `use super::notifications::notification_settings;` or similar.

Migrate associated tests.

- [ ] **Step 3: Declare submodules and update dispatch**

```rust
mod activity;
mod notifications;
```

Update `handle_event`:

```rust
ActivityLog { tool_name, tool_input, tool_response } =>
    activity::handle_activity_log(pane, &tool_name, &tool_input, &tool_response),
```

And anywhere `notification_settings()` is called, use `notifications::notification_settings()`.

- [ ] **Step 4: Verification checklist + commit**

```bash
cargo fmt && cargo fmt --check && cargo clippy && cargo test && cargo insta pending-snapshots
git add -A
git commit -m "refactor(cli/hook): split activity and notifications modules"
```

After this task, `src/cli/hook.rs` should be ≤ ~200 lines: just the CLI entry, dispatch, and module declarations.

---

## Chunk 5: New Tests and Documentation (Commits 12–13)

### Task 5.1: Add unit tests for extracted units

**Goal:** Achieve 100% function coverage for every function/struct created or newly-exposed during this refactor.

**Files:**
- Modify: `src/state/activity.rs`, `src/state/session.rs`, `src/state/focus.rs`, `src/state/scroll.rs`, `src/state/pane_runtime.rs`
- Modify: `src/ui/panes.rs` (for `PaneLayout::compute`)
- Modify: `src/ui/panes/row_collector.rs`, `src/ui/panes/click_targets.rs`, `src/ui/panes/popups.rs`
- Modify: `src/cli/hook/context.rs`, `src/cli/hook/handlers.rs`
- Modify: `src/tmux.rs` (for named-constant parse regression test)

Tests in this task verify behavior directly. Each follows TDD: write a failing test referencing the new unit, then confirm it passes without modification (the implementation already exists from prior tasks).

For each unit below: read the source, identify its public API (constructors, single public function, etc.), write a minimal test covering the happy path and at least one edge case.

- [ ] **Step 1: `ActivityState`, `SessionNamesState`, `FocusState`, `ScrollStates`, `PaneRuntimeMap` constructors and delegation methods**

For each: add tests verifying `Default` or `new()` produces the expected initial state, and that public methods (e.g., `PaneRuntimeMap::get`, `PaneRuntimeMap::entry_mut`, `PaneRuntimeMap::contains_key`, `PaneRuntimeMap::remove`) behave as delegated.

Example for `PaneRuntimeMap`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_mut_creates_default_on_miss() {
        let mut map = PaneRuntimeMap::new();
        let state = map.entry_mut("pane-1");
        assert!(state.ports.is_empty());
        assert!(state.command.is_none());
    }

    #[test]
    fn get_returns_none_before_entry() {
        let map = PaneRuntimeMap::new();
        assert!(map.get("pane-1").is_none());
    }

    #[test]
    fn remove_returns_the_prior_value() {
        let mut map = PaneRuntimeMap::new();
        map.entry_mut("pane-1").ports = vec![8080];
        let removed = map.remove("pane-1").unwrap();
        assert_eq!(removed.ports, vec![8080]);
        assert!(map.get("pane-1").is_none());
    }
}
```

- [ ] **Step 2: `PaneLayout::compute`**

```rust
#[cfg(test)]
mod tests {
    // ... inside src/ui/panes.rs test module ...

    #[test]
    fn pane_layout_splits_area_into_filter_secondary_list() {
        let area = Rect { x: 0, y: 0, width: 40, height: 20 };
        let layout = PaneLayout::compute(area);
        assert_eq!(layout.filter_area.height, 1);
        assert_eq!(layout.secondary_area.y, 1);
        assert_eq!(layout.secondary_area.height, 1);
        assert_eq!(layout.list_area.y, 2);
        assert_eq!(layout.list_area.height, 18);
    }

    #[test]
    fn pane_layout_handles_tiny_area() {
        let area = Rect { x: 0, y: 0, width: 40, height: 1 };
        let layout = PaneLayout::compute(area);
        assert_eq!(layout.filter_area.height, 1);
        assert_eq!(layout.secondary_area.height, 0);
        assert_eq!(layout.list_area.height, 0);
    }

    #[test]
    fn pane_layout_handles_zero_height() {
        let area = Rect { x: 0, y: 0, width: 40, height: 0 };
        let layout = PaneLayout::compute(area);
        assert_eq!(layout.filter_area.height, 0);
        assert_eq!(layout.secondary_area.height, 0);
        assert_eq!(layout.list_area.height, 0);
    }
}
```

- [ ] **Step 3: `row_collector::collect`**

Fixtures: construct `AppState` with one, two, and three `RepoGroup` entries. Apply different `StatusFilter` and `RepoFilter` values. Assert `CollectedRows.lines.len()`, `pending_spawn.len()`, `pending_remove.len()`. No snapshot tests here — those are already covered by `tests/ui_snapshot.rs`.

- [ ] **Step 4: `click_targets::materialize`**

Fixture: a `CollectedRows` with known `line_to_row` and pending vectors, plus `scroll_offset = 3`. Call `materialize` and assert `state.layout.pane_row_targets` contains the expected absolute `Rect` entries.

- [ ] **Step 5: `make_ctx` construction**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_ctx_wires_all_fields() {
        let agent = "claude".to_string();
        let cwd = "/tmp".to_string();
        let pm = "auto".to_string();
        let worktree = None;
        let sid = None;
        let ctx = make_ctx(&agent, &cwd, &pm, &worktree, &sid);
        assert_eq!(ctx.agent, "claude");
        assert_eq!(ctx.cwd, "/tmp");
        assert_eq!(ctx.permission_mode, "auto");
        assert!(ctx.worktree.is_none());
        assert!(ctx.session_id.is_none());
    }
}
```

- [ ] **Step 6: Hook handler tests (per variant)**

For each `on_*` handler that is not already tested (grep `fn on_` in `src/cli/hook/handlers.rs` and cross-reference against existing tests after migration), add a table-style test that:
1. Sets up the tmux option state needed (via `tmux::set_pane_option` with a test fixture pane, or by stubbing — follow the pattern already used in existing hook tests).
2. Calls the handler.
3. Asserts the expected tmux options are set/unset and the return code is 0.

If no existing test pattern exists for stubbing tmux in unit tests, skip the handler — do not add a new mock layer (out of scope per spec).

- [ ] **Step 7: `parse_pane_line` regression test after named constants**

First locate the format string that feeds `parse_pane_line` (so the fixture matches field-for-field):

```bash
rg -n 'list-panes' src/tmux.rs
```

Find the `-F "..."` format argument. Each `#{...}` placeholder is a `|`-separated field. Build the fixture by concatenating representative values in the exact same order.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pane_line_with_named_constants_matches_prior_fixture() {
        // Field order (check against the -F format string in src/tmux.rs):
        //   0: session_name  3: agent_type  5: pane_current_path
        //   6: pane_current_command  7: window_name  9: prompt
        //   10: prompt_source  13: pane_pid  15: @pane_cwd
        //   16: @pane_permission_mode  19: @pane_session_id
        //   (complete list: use pane_field::* constants)
        //
        // parse_pane_line receives the line already stripped of the
        // first 6 "window" fields, so indices above are absolute from
        // the full format string; within parse_pane_line they are
        // offset by -6.
        let fields = [
            // ... fill in one representative value per field ...
        ];
        let line = fields.join("|");
        let pane = parse_pane_line(&line).unwrap();
        assert_eq!(pane.pane_id, "%42");
        assert_eq!(pane.worktree.name, "feat-branch");
        // ... one assert per field a named constant now indexes into ...
    }
}
```

Alternative: reuse a fixture builder from `tests/test_helpers.rs` if one exists for `PaneInfo` / the raw tmux line. Grep `rg -n 'fn.*PaneInfo|fn.*pane_line' tests/test_helpers.rs` to check.

- [ ] **Step 8: Run full verification + commit**

```bash
cargo fmt && cargo fmt --check && cargo clippy && cargo test && cargo insta pending-snapshots
```

Optional: measure coverage delta if `cargo-llvm-cov` is installed:

```bash
cargo llvm-cov --summary-only --html
# Spot-check that function coverage on extracted units is 100%.
```

```bash
git add -A
git commit -m "test: add unit tests for extracted units"
```

---

### Task 5.2: Update `CLAUDE.md` and `docs/state-management.md`

**Goal:** Reflect the new module structure in the two architecture documents.

**Files:**
- Modify: `CLAUDE.md`
- Modify: `docs/state-management.md`

- [ ] **Step 1: Update `CLAUDE.md` Key Modules section**

Find the `### Key Modules` section. Update the `state.rs` bullet to reference the new `state/` submodule layout:

> - **`state.rs` + `state/`** — `AppState` central struct + topical submodules (`activity`, `focus`, `scroll`, `popup`, `notices`, `timers`, `pane_runtime`, `session`, `global`, `filter`, `layout`, `tab`). All UI is computed from this state.

Update `ui/` bullet to mention the new `panes/` submodules (`filter_bar`, `row_collector`, `click_targets`, `popups`).

Update the `cli/hook.rs` bullet to note the new `cli/hook/` submodules (`context`, `handlers`, `activity`, `notifications`).

- [ ] **Step 2: Update `docs/state-management.md`**

Two changes:
1. Reflect the new sub-struct layout on `AppState`. Replace any field list that enumerates flat fields (`activity_entries`, `focused_pane_id`, etc.) with a description of the sub-struct hierarchy.
2. Fix the pre-existing drift in the `FrameLayout` listing at lines 85–86: it currently lists only `pane_row_targets, line_to_row, repo_button_col, hyperlink_overlays`. Add the missing `repo_spawn_targets` and `spawn_remove_targets` fields.

- [ ] **Step 3: Verify + commit**

```bash
cargo fmt --check     # sanity
git add -A
git commit -m "docs: update architecture sections for refactored module layout"
```

---

## Pre-Merge Checklist

Run once the whole 13-commit sequence is in:

- [ ] `cargo fmt --check` — passes
- [ ] `cargo clippy` — passes
- [ ] `cargo test` — 912+ existing tests plus new tests from Task 5.1 all pass
- [ ] `cargo build --release` — succeeds
- [ ] `cargo insta pending-snapshots` — empty output
- [ ] Manual smoke test in live tmux:
  - [ ] Each `PaneStatus` (Running / Waiting / Idle / Error) renders with the correct icon and color
  - [ ] Activity log / GitStatus tab switching works
  - [ ] Spawn popup opens, accepts input, creates a worktree window
  - [ ] Remove popup opens, confirms, removes the pane's worktree
  - [ ] Notices popup opens (ⓘ button) and shows current hook diagnostic state
  - [ ] Status filter cycles through All / Running / Waiting / Idle / Error
  - [ ] Repo filter cycles between All and a selected repo
  - [ ] Agent list scrolls
  - [ ] Activity log scrolls
  - [ ] Hook events from Claude Code (`session-start`, `stop`, `notification`, `task-completed`) update the sidebar in real time
  - [ ] Hook events from Codex update the sidebar
  - [ ] Sidebar runs for ≥5 minutes without crash or layout corruption

## Rollback

If anything breaks mid-PR and bisecting identifies a bad commit:

```bash
git reset --soft HEAD~N   # walk back to the last green commit
```

Each commit is `cargo test`-green independently — re-examine the faulty step in isolation.
