# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

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
1. **TUI mode** (`src/main.rs`) — default, renders the sidebar UI
2. **CLI subcommands** — `hook`, `toggle`, `auto-close`, `set-status`, `--version`

### Core Data Flow

```
Agent hooks (hook.sh) → CLI `hook` subcommand → writes to /tmp/tmux-agent-sidebar-*
                                                        ↓
TUI event loop (main.rs) → AppState::sync_global_state() → reads tmux panes + /tmp files
                                                        ↓
                                            ui::draw() renders frame
```

### Key Modules

- **`state.rs`** — `AppState` central struct: sessions, repo groups, filters, scroll positions, focus management. All UI is computed from this state.
- **`tmux.rs`** — Tmux integration: queries all panes via single `list-panes -a` call, defines `PaneInfo`/`PaneStatus`/`AgentType`/`PermissionMode`.
- **`cli/hook.rs`** — Receives real-time status updates from agent hooks, writes state to `/tmp/` files for the TUI to read.
- **`git.rs`** — Git operations (branch, ahead/behind, PR numbers via `gh` CLI, diff stats). Runs in a background polling thread.
- **`activity.rs`** — Parses `/tmp/tmux-agent-activity*.log` files, maps tool types to colors.
- **`group.rs`** — Groups panes by repository path.
- **`ui/`** — Rendering layer: `panes.rs` (agent list + repo filter), `bottom.rs` (activity/git tabs), `colors.rs` (256-color theme), `text.rs` (text formatting/truncation).

### State Management

- `Focus` enum: Filter, Panes, ActivityLog — controls keyboard input routing
- `StatusFilter`: All, Running, Waiting, Idle, Error
- `BottomTab`: Activity, GitStatus
- SIGUSR1 signal triggers instant refresh on tmux pane focus change

### Testing

Tests are in `/tests/` using Ratatui's `TestBackend` for UI rendering assertions. `test_helpers.rs` provides buffer-to-string conversion utilities. Heavy use of snapshot-style tests for UI regression prevention.

## Debugging (Local tmux Plugin)

`~/.tmux/plugins/tmux-agent-sidebar` is typically a symlink to this repository, so `cargo build --release` alone updates the binary tmux loads. Just restart the sidebar (toggle off → on via the tmux keybinding) to pick up the new build.

```bash
cargo build --release
# Restart sidebar (toggle off → on via tmux keybinding)
```

**When working in a worktree**: Worktrees build into their own `target/release/`, which is not what the plugin directory points at, so the artifact must be copied manually.

```bash
cp <worktree-path>/target/release/tmux-agent-sidebar ~/.tmux/plugins/tmux-agent-sidebar/target/release/tmux-agent-sidebar
```

## Rust Edition

This project uses Rust edition 2024 (`Cargo.toml`).

## Writing Guidelines

- All documentation under `docs/` and all skill files under `.claude/skills/` must be written in English.
