use super::location::sync_worktree_meta;
use super::meta::clear_all_meta;
use crate::cli::{set_attention, set_status};
use crate::tmux;

/// Tmux pane option set when SessionEnd is deferred because subagents are
/// still active. Drained by `on_subagent_stop` once `@pane_subagents`
/// becomes empty.
pub(in crate::cli::hook) const PENDING_SESSION_END: &str = "@pane_pending_session_end";
/// Same idea for WorktreeRemove.
pub(in crate::cli::hook) const PENDING_WORKTREE_REMOVE: &str = "@pane_pending_worktree_remove";

pub(in crate::cli::hook) fn mark_pending(pane: &str, key: &str) {
    tmux::set_pane_option(pane, key, "1");
}

/// Run any deferred teardowns recorded by previous calls to
/// `on_session_end` / `on_worktree_remove`. Called from `on_subagent_stop`
/// after the subagent list drains to empty so the parent pane is finally
/// cleaned up instead of being stranded with stale metadata.
pub(in crate::cli::hook) fn drain_pending_teardowns(pane: &str) {
    let pending_session_end = !tmux::get_pane_option_value(pane, PENDING_SESSION_END).is_empty();
    let pending_worktree_remove =
        !tmux::get_pane_option_value(pane, PENDING_WORKTREE_REMOVE).is_empty();

    if pending_session_end {
        // SessionEnd already cleared the pending marker via clear_all_meta.
        run_session_end_teardown(pane);
    } else if pending_worktree_remove {
        run_worktree_remove_teardown(pane);
        tmux::unset_pane_option(pane, PENDING_WORKTREE_REMOVE);
    }
}

/// Side-effect body of the SessionEnd teardown. Extracted so both the
/// inline path (no subagents) and the deferred path (drained from
/// `on_subagent_stop`) execute the exact same cleanup.
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
