use super::location::sync_worktree_meta;
use super::meta::clear_all_meta;
use crate::cli::{set_attention, set_status};
use crate::tmux;

/// Legacy pane-option name. Nothing sets this any more — a child
/// SessionEnd can't be distinguished from the parent's, so the
/// deferred-teardown dance was too dangerous to keep. The constant is
/// retained so `clear_all_meta` and `on_session_start` can still sweep
/// a stale marker left behind by a pre-fix install.
pub(in crate::cli::hook) const PENDING_SESSION_END: &str = "@pane_pending_session_end";
/// Tmux pane option set when WorktreeRemove is deferred because
/// subagents are still active. Drained by `on_subagent_stop` once
/// `@pane_subagents` becomes empty.
pub(in crate::cli::hook) const PENDING_WORKTREE_REMOVE: &str = "@pane_pending_worktree_remove";

pub(in crate::cli::hook) fn mark_pending(pane: &str, key: &str) {
    tmux::set_pane_option(pane, key, "1");
}

/// Run any deferred teardown recorded by a previous call to
/// `on_worktree_remove`. Called from `on_subagent_stop` after the
/// subagent list drains to empty so the parent pane is finally cleaned
/// up instead of being stranded with stale metadata.
///
/// `on_session_end` no longer participates — a child SessionEnd racing
/// ahead of SubagentStop would otherwise replay the teardown against
/// the live parent. The SessionEnd path now bails out early instead.
pub(in crate::cli::hook) fn drain_pending_teardowns(pane: &str) {
    if !tmux::get_pane_option_value(pane, PENDING_WORKTREE_REMOVE).is_empty() {
        run_worktree_remove_teardown(pane);
        tmux::unset_pane_option(pane, PENDING_WORKTREE_REMOVE);
    }
}

/// Side-effect body of the SessionEnd teardown. Invoked by
/// `on_session_end` when no subagents are active; subagent-active
/// SessionEnds are short-circuited before they reach this point.
pub(in crate::cli::hook) fn run_session_end_teardown(pane: &str) {
    set_attention(pane, "clear");
    clear_all_meta(pane);
    set_status(pane, "clear");
    let log_path = crate::activity::log_file_path(pane);
    let _ = std::fs::remove_file(log_path);
}

/// Side-effect body of the WorktreeRemove teardown. Same pattern as
/// `run_session_end_teardown` — single source of truth for both the inline
/// and deferred paths.
pub(in crate::cli::hook) fn run_worktree_remove_teardown(pane: &str) {
    sync_worktree_meta(pane, &None);
    // Clear hook-set cwd so query_sessions() falls back to
    // pane_current_path, avoiding stale worktree path association.
    tmux::unset_pane_option(pane, "@pane_cwd");
}
