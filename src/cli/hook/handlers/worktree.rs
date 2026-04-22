use super::super::context::{
    PENDING_WORKTREE_REMOVE, mark_pending, pane_writes_allowed, run_worktree_remove_teardown,
};

pub(in crate::cli::hook) fn on_worktree_remove(pane: &str) -> i32 {
    // If subagents are active, the removed worktree may belong to one of
    // them — we can't distinguish parent from child at this point, so the
    // safe default is to leave the parent's pane-scoped metadata intact.
    // Record the intent and let `on_subagent_stop` execute it once
    // children are gone.
    if !pane_writes_allowed(pane) {
        mark_pending(pane, PENDING_WORKTREE_REMOVE);
        return 0;
    }
    run_worktree_remove_teardown(pane);
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmux;

    #[test]
    fn on_worktree_remove_preserves_parent_state_when_subagents_active() {
        let _guard = tmux::test_mock::install();
        let pane = "%PARENT_WT";
        tmux::test_mock::set(pane, "@pane_subagents", "Explore:sub-1");
        tmux::test_mock::set(pane, "@pane_worktree_name", "parent-feat");
        tmux::test_mock::set(pane, "@pane_worktree_branch", "feat/parent");
        tmux::test_mock::set(pane, "@pane_cwd", "/repo/parent");

        on_worktree_remove(pane);

        assert_eq!(
            tmux::test_mock::get(pane, "@pane_worktree_name").as_deref(),
            Some("parent-feat")
        );
        assert_eq!(
            tmux::test_mock::get(pane, "@pane_worktree_branch").as_deref(),
            Some("feat/parent")
        );
        assert_eq!(
            tmux::test_mock::get(pane, "@pane_cwd").as_deref(),
            Some("/repo/parent")
        );
    }

    #[test]
    fn on_worktree_remove_clears_state_when_no_subagents() {
        let _guard = tmux::test_mock::install();
        let pane = "%LONE_WT";
        tmux::test_mock::set(pane, "@pane_worktree_name", "old");
        tmux::test_mock::set(pane, "@pane_worktree_branch", "old");
        tmux::test_mock::set(pane, "@pane_cwd", "/wt/old");

        on_worktree_remove(pane);

        assert!(!tmux::test_mock::contains(pane, "@pane_worktree_name"));
        assert!(!tmux::test_mock::contains(pane, "@pane_worktree_branch"));
        assert!(!tmux::test_mock::contains(pane, "@pane_cwd"));
    }

    #[test]
    fn on_worktree_remove_defers_via_pending_marker_under_subagents() {
        let _guard = tmux::test_mock::install();
        let pane = "%PENDING_WT";
        tmux::test_mock::set(pane, "@pane_subagents", "Explore:sub-1");

        on_worktree_remove(pane);

        assert!(
            tmux::test_mock::contains(pane, PENDING_WORKTREE_REMOVE),
            "pending marker must be set when subagents are active"
        );
    }
}
