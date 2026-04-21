## Project Overview

A tmux sidebar TUI (built with Ratatui + Crossterm) that monitors AI coding agents (Claude Code, Codex) across all tmux sessions/windows/panes in real-time. Distributed as a single binary via tmux plugin managers.

## Build & Development Commands

```bash
cargo build                    # Debug build
cargo build --release          # Release build (strip + lto enabled)
cargo test                     # Run all tests
cargo test <test_name>         # Run a single test
cargo clippy                   # Lint
cargo fmt                      # Format code
cargo fmt --check              # Check formatting (used in CI)
```

CI runs `cargo test`, `cargo clippy`, and `cargo fmt --check` on every push/PR.

**Before creating any git commit**, always run `cargo fmt` first to avoid CI formatting failures. This applies to every commit, not just the final one.

After implementation is complete, run `cargo build --release`. The plugin directory is usually a symlink to this repo, so the binary is picked up automatically; only a worktree build needs a manual copy (see "Debugging" section below).

## Architecture

### Entry Points

The binary has two modes controlled by CLI args (`src/cli/mod.rs`):
1. **TUI mode** — default. `src/main.rs` handles CLI arg parsing, SIGUSR1 signal wiring, and TUI session setup, then delegates to `app::run` (`src/app.rs`) for the event loop.
2. **CLI subcommands** — `setup`, `hook`, `toggle`, `toggle-all`, `auto-close`, `set-status`, `spawn`, `capture`, `--version` / `version`.

### Core Data Flow

```
Agent hooks (hook.sh) → CLI `hook` subcommand
                           ↓
        adapter/ normalizes raw JSON into AgentEventKind
                           ↓
        event/ builds an internal AgentEvent
                           ↓
        cli/hook/handlers dispatches on_* per event, which:
          • sets tmux pane options (@pane_status, @pane_attention, etc.)
          • appends to /tmp/tmux-agent-activity*.log
                           ↓
TUI event loop (app::run) → AppState::sync_global_state()
          • reads tmux panes via single `list-panes -a`
          • parses /tmp/tmux-agent-activity*.log
                           ↓
                ui::draw() renders frame
```

### Key Modules

- **`state.rs` + `state/`** — `AppState` central struct plus topical submodules (`activity`, `session`, `focus`, `scroll`, `pane_runtime`, `layout`, `popup`, `notices`, `timers`, `filter`, `global`, `refresh`, `tab`). All UI is computed from this state.
- **`app.rs` + `app/`** — TUI orchestration: `setup` (prime `AppState`), `workers` (background git/session/version threads), `input` (keyboard/mouse handling), `render` (per-frame render entry). Split out from `main.rs` so the binary entry point only handles CLI dispatch, signal wiring, and TUI session setup.
- **`tmux.rs`** — Tmux integration: queries all panes via single `list-panes -a` call, defines `PaneInfo`/`PaneStatus`/`AgentType`/`PermissionMode`/`WorktreeMetadata`.
- **`adapter/`** — Per-agent hook adapters (`claude`, `codex`, `opencode`). Each exposes a `HOOK_REGISTRATIONS` table binding upstream hook triggers to an internal `AgentEventKind`, plus a `parse()` that maps raw JSON payloads into `AgentEvent`. Single source of truth consumed by the setup wizard, README snippets, and tests.
- **`event.rs` + `event/`** — Internal event layer: `AgentEvent` (pre-extracted fields; handlers never touch raw JSON or agent names), `AgentEventKind` (compile-time enum for hook kinds), `EventAdapter` trait + `resolve_adapter`.
- **`cli/hook.rs` + `cli/hook/`** — Receives real-time status updates from agent hooks; dispatch in `hook.rs`, with submodules `context` (shared helpers + `AgentContext`), `handlers` (per-event `on_*` handlers), `activity` (activity log writing), `notifications` (desktop notification helpers).
- **`git.rs`** — Git operations (branch, ahead/behind, PR numbers via `gh` CLI, diff stats). Runs in a background polling thread.
- **`activity.rs`** — Parses `/tmp/tmux-agent-activity*.log` files, maps tool types to colors.
- **`group.rs`** — Groups panes by repository path.
- **`session.rs` / `worktree.rs` / `tool_name.rs` / `version.rs` / `port.rs` / `clipboard.rs` / `desktop_notification.rs`** — Leaf helpers used across modules (session name resolution, worktree metadata parsing, tool-name classification, version reporting, port detection, clipboard + desktop notification shims).
- **`ui/`** — Rendering layer: `mod.rs` (entry `draw`), `panes.rs` (agent list + repo filter) with submodules (`filter_bar`, `row`, `row_collector`, `click_targets`, `popups`); `bottom.rs` + `bottom/` with submodules (`activity`, `git`) for the activity/git tabs; `colors.rs` (256-color theme); `icons.rs` (agent/status glyphs); `notices.rs` (transient banner rendering); `text.rs` (text formatting/truncation).

### State Management

See `docs/state-management.md` for the full scope/update-frequency table, per-pane tmux options, data flow, and key type definitions.

Process-level detail not covered there: SIGUSR1 triggers an instant refresh on tmux pane focus change (handler in `src/main.rs` flips a shared `AtomicBool` that the `app::run` loop polls).

### Testing

Tests are in `/tests/` using Ratatui's `TestBackend` for UI rendering assertions. `test_helpers.rs` provides buffer-to-string conversion utilities. Heavy use of snapshot-style tests for UI regression prevention.

**UI test rule**: any test that renders a frame MUST use `insta::assert_snapshot!(output, @"...")` inline snapshots — never `assert!(output.contains(...))` or similar substring checks. A contains assertion only verifies that a specific string appears somewhere; it silently tolerates layout drift (border shifts, color changes, row reordering, new artifacts) that a snapshot diff would surface immediately. The stronger check is free — `cargo insta accept` regenerates the expected output when the change is intentional. Substring assertions are acceptable only for non-visual properties (`layout.repo_spawn_targets` contents, state struct fields, etc.) where there is no frame to snapshot.

## Debugging (Local tmux Plugin)

`~/.tmux/plugins/tmux-agent-sidebar` is typically a symlink to this repository, so `cargo build --release` alone updates the binary tmux loads. Just restart the sidebar (toggle off → on via the tmux keybinding) to pick up the new build.

```bash
cargo build --release
# Restart sidebar (toggle off → on via tmux keybinding)
```

**When working in a worktree**: Worktrees build into their own `target/release/`, which is not what the plugin directory points at, so the artifact must be copied manually AND re-signed. On macOS (Darwin 24+), `cargo` produces a `linker-signed` ad-hoc signature that the kernel will SIGKILL (signal 9) immediately after a `cp` — the kernel refuses to honor a linker-only signature on a file it didn't write itself. Replace it with a fresh ad-hoc signature to avoid the kill:

```bash
cp <worktree-path>/target/release/tmux-agent-sidebar ~/.tmux/plugins/tmux-agent-sidebar/target/release/tmux-agent-sidebar
codesign --force --sign - ~/.tmux/plugins/tmux-agent-sidebar/target/release/tmux-agent-sidebar
```

If tmux reports `terminated by signal 9` after a worktree build, you almost certainly skipped the `codesign` step. Clearing `com.apple.provenance` with `xattr -c` is not required — the kernel only cares about the signature flavor.

## Rust Edition

This project uses Rust edition 2024 (`Cargo.toml`).

## Writing Guidelines

- All documentation under `docs/` and all skill files under `.claude/skills/` must be written in English.
