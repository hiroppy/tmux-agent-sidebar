//! Spawn / remove flow for the sidebar `n` / `x` keybindings. Owns the
//! handful of writes (git worktree, tmux new-window) that turn this
//! otherwise read-only sidebar into a worktree multiplexer.

mod config;
mod env;
mod flow;
mod markers;
mod slug;

pub use config::{
    AGENT_OPTION, AGENTS, BRANCH_PREFIX_OPTION, CLAUDE_MODES, CODEX_MODES, DEFAULT_AGENT,
    DEFAULT_BRANCH_PREFIX, DEFAULT_MODE, DEFAULT_WORKTREE_DIR, OPENCODE_MODES, PI_MODES,
    RemoveMode, WORKTREE_DIR_OPTION, agent_command, modes_for,
};
pub use flow::{SpawnRequest, remove, spawn};
pub use markers::{
    SPAWNED_BRANCH_OPTION, SPAWNED_FROM_OPTION, SPAWNED_OPTION, SPAWNED_WORKTREE_OPTION,
    SpawnMarkers, read_spawn_markers, spawn_markers_template,
};
pub use slug::{pick_unique_slug, slugify, worktree_path_for};
