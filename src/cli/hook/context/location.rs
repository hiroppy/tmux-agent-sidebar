use crate::event::WorktreeInfo;
use crate::tmux;

/// Returns whether the pane's cwd should be updated.
/// When subagents are active, events may come from a subagent running in a
/// worktree, so we should NOT overwrite the parent agent's cwd.
pub(in crate::cli::hook) fn should_update_cwd(current_subagents: &str) -> bool {
    current_subagents.is_empty()
}

/// Resolve the effective cwd for pane metadata.
/// When a worktree is active, prefer `original_repo_dir` so the sidebar
/// groups the pane under the original repository, not the worktree path.
pub(in crate::cli::hook) fn resolve_cwd<'a>(
    raw_cwd: &'a str,
    worktree: &'a Option<WorktreeInfo>,
) -> &'a str {
    if let Some(wt) = worktree
        && !wt.original_repo_dir.is_empty()
    {
        return &wt.original_repo_dir;
    }
    raw_cwd
}

/// Sync worktree name/branch pane options from hook payload.
///
/// Clears both options when worktree is `None`. When worktree is `Some`
/// but an individual field is empty, also clears that field so an
/// explicit "no longer in a worktree" payload cannot leave stale
/// metadata from a previous run behind.
pub(in crate::cli::hook) fn sync_worktree_meta(pane: &str, worktree: &Option<WorktreeInfo>) {
    if let Some(wt) = worktree {
        if wt.name.is_empty() {
            tmux::unset_pane_option(pane, "@pane_worktree_name");
        } else {
            tmux::set_pane_option(pane, "@pane_worktree_name", &wt.name);
        }
        if wt.branch.is_empty() {
            tmux::unset_pane_option(pane, "@pane_worktree_branch");
        } else {
            tmux::set_pane_option(pane, "@pane_worktree_branch", &wt.branch);
        }
    } else {
        tmux::unset_pane_option(pane, "@pane_worktree_name");
        tmux::unset_pane_option(pane, "@pane_worktree_branch");
    }
}

pub(in crate::cli::hook) fn sync_pane_location(
    pane: &str,
    cwd: &str,
    worktree: &Option<WorktreeInfo>,
    session_id: &Option<String>,
) {
    // Subagents share the parent's $TMUX_PANE and can fire their own hook
    // events with a different session_id, cwd, or worktree. While children
    // are active, every pane-scoped write must be skipped so the parent's
    // identity is preserved — including `@pane_worktree_*`, which used to
    // leak through and misgroup the pane under the child's repo.
    let current_subagents = tmux::get_pane_option_value(pane, "@pane_subagents");
    if !should_update_cwd(&current_subagents) {
        return;
    }
    match session_id.as_deref() {
        Some(sid) if !sid.is_empty() => tmux::set_pane_option(pane, "@pane_session_id", sid),
        _ => tmux::unset_pane_option(pane, "@pane_session_id"),
    }
    if !cwd.is_empty() {
        let effective_cwd = resolve_cwd(cwd, worktree);
        tmux::set_pane_option(pane, "@pane_cwd", effective_cwd);
    }
    sync_worktree_meta(pane, worktree);
}

/// Returns true if pane-scoped writes from this hook event are safe to
/// apply to the pane's metadata. False while subagents are active so a
/// child hook cannot clobber the parent pane's identity.
pub(in crate::cli::hook) fn pane_writes_allowed(pane: &str) -> bool {
    let current_subagents = tmux::get_pane_option_value(pane, "@pane_subagents");
    should_update_cwd(&current_subagents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_cwd_prefers_worktree_original_repo_dir() {
        let wt = WorktreeInfo {
            name: "feat".into(),
            path: "/tmp/wt".into(),
            branch: "feat".into(),
            original_repo_dir: "/home/user/repo".into(),
        };
        assert_eq!(resolve_cwd("/tmp/wt/src", &Some(wt)), "/home/user/repo");
    }

    #[test]
    fn resolve_cwd_falls_back_to_raw_cwd() {
        assert_eq!(resolve_cwd("/tmp/project", &None), "/tmp/project");
    }

    #[test]
    fn resolve_cwd_worktree_empty_original_falls_back() {
        let wt = WorktreeInfo {
            name: "feat".into(),
            path: "/tmp/wt".into(),
            branch: "feat".into(),
            original_repo_dir: "".into(),
        };
        assert_eq!(resolve_cwd("/tmp/wt/src", &Some(wt)), "/tmp/wt/src");
    }

    #[test]
    fn should_update_cwd_when_no_subagents() {
        assert!(should_update_cwd(""));
    }

    #[test]
    fn should_not_update_cwd_when_subagent_active() {
        assert!(!should_update_cwd("Explore:sub-1"));
    }

    #[test]
    fn should_not_update_cwd_when_multiple_subagents_active() {
        assert!(!should_update_cwd("Explore:sub-1,Plan:sub-2"));
    }

    #[test]
    fn sync_pane_location_skips_worktree_writes_while_subagents_active() {
        let _guard = tmux::test_mock::install();
        let pane = "%PARENT";
        // Parent state: real worktree owned by the parent agent.
        tmux::test_mock::set(pane, "@pane_subagents", "Explore:sub-1");
        tmux::test_mock::set(pane, "@pane_worktree_name", "parent-feat");
        tmux::test_mock::set(pane, "@pane_worktree_branch", "feat/parent");
        tmux::test_mock::set(pane, "@pane_cwd", "/repo/parent");
        tmux::test_mock::set(pane, "@pane_session_id", "parent-session");

        // Subagent fires a hook with its own (different) worktree.
        let child_wt = Some(WorktreeInfo {
            name: "child-feat".into(),
            path: "/wt/child".into(),
            branch: "feat/child".into(),
            original_repo_dir: "/repo/child".into(),
        });
        sync_pane_location(
            pane,
            "/repo/child",
            &child_wt,
            &Some("child-session".into()),
        );

        assert_eq!(
            tmux::test_mock::get(pane, "@pane_worktree_name").as_deref(),
            Some("parent-feat"),
            "worktree name must not leak from subagent into parent"
        );
        assert_eq!(
            tmux::test_mock::get(pane, "@pane_worktree_branch").as_deref(),
            Some("feat/parent")
        );
        assert_eq!(
            tmux::test_mock::get(pane, "@pane_cwd").as_deref(),
            Some("/repo/parent")
        );
        assert_eq!(
            tmux::test_mock::get(pane, "@pane_session_id").as_deref(),
            Some("parent-session")
        );
    }

    #[test]
    fn sync_pane_location_writes_worktree_when_no_subagents() {
        let _guard = tmux::test_mock::install();
        let pane = "%LONE";
        let wt = Some(WorktreeInfo {
            name: "feat-x".into(),
            path: "/wt/feat-x".into(),
            branch: "feat-x".into(),
            original_repo_dir: "/repo".into(),
        });

        sync_pane_location(pane, "/wt/feat-x", &wt, &Some("sess-1".into()));

        assert_eq!(
            tmux::test_mock::get(pane, "@pane_worktree_name").as_deref(),
            Some("feat-x")
        );
        assert_eq!(
            tmux::test_mock::get(pane, "@pane_worktree_branch").as_deref(),
            Some("feat-x")
        );
        // resolve_cwd routes the original_repo_dir into @pane_cwd.
        assert_eq!(
            tmux::test_mock::get(pane, "@pane_cwd").as_deref(),
            Some("/repo")
        );
        assert_eq!(
            tmux::test_mock::get(pane, "@pane_session_id").as_deref(),
            Some("sess-1")
        );
    }

    #[test]
    fn sync_worktree_meta_clears_when_worktree_is_none() {
        let _guard = tmux::test_mock::install();
        let pane = "%CLEAR";
        tmux::test_mock::set(pane, "@pane_worktree_name", "old");
        tmux::test_mock::set(pane, "@pane_worktree_branch", "feat/old");

        sync_worktree_meta(pane, &None);

        assert!(!tmux::test_mock::contains(pane, "@pane_worktree_name"));
        assert!(!tmux::test_mock::contains(pane, "@pane_worktree_branch"));
    }

    #[test]
    fn sync_worktree_meta_clears_individual_empty_fields() {
        // Regression: a Some(worktree) payload with only `name`
        // populated must still drop a stale `@pane_worktree_branch`
        // that a previous run set — otherwise the sidebar keeps
        // rendering the old branch.
        let _guard = tmux::test_mock::install();
        let pane = "%PARTIAL";
        tmux::test_mock::set(pane, "@pane_worktree_name", "old-name");
        tmux::test_mock::set(pane, "@pane_worktree_branch", "feat/old");

        sync_worktree_meta(
            pane,
            &Some(WorktreeInfo {
                name: "new-name".into(),
                path: "/wt/new".into(),
                branch: String::new(),
                original_repo_dir: "/repo".into(),
            }),
        );

        assert_eq!(
            tmux::test_mock::get(pane, "@pane_worktree_name").as_deref(),
            Some("new-name"),
            "non-empty name must overwrite"
        );
        assert!(
            !tmux::test_mock::contains(pane, "@pane_worktree_branch"),
            "empty branch must clear the stale option"
        );
    }

    #[test]
    fn pane_writes_allowed_tracks_subagent_presence() {
        let _guard = tmux::test_mock::install();
        let pane = "%ALLOWED";
        assert!(pane_writes_allowed(pane));
        tmux::test_mock::set(pane, "@pane_subagents", "Explore:sub-1");
        assert!(!pane_writes_allowed(pane));
    }
}
