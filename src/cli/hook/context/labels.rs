use super::location::resolve_cwd;
use super::meta::AgentContext;
use crate::tmux;

pub(in crate::cli::hook) fn repo_label_from_ctx(ctx: &AgentContext<'_>) -> Option<String> {
    let cwd = resolve_cwd(ctx.cwd, ctx.worktree);
    repo_label_from_path(cwd)
}

pub(in crate::cli::hook) fn repo_label_from_pane(pane: &str) -> Option<String> {
    let cwd = tmux::get_pane_option_value(pane, "@pane_cwd");
    if !cwd.is_empty() {
        return repo_label_from_path(&cwd);
    }
    let worktree = tmux::get_pane_option_value(pane, "@pane_worktree_name");
    if !worktree.is_empty() {
        return Some(worktree);
    }
    None
}

pub(in crate::cli::hook) fn branch_label_from_ctx(ctx: &AgentContext<'_>) -> Option<String> {
    if let Some(wt) = ctx.worktree
        && !wt.branch.is_empty()
    {
        return Some(wt.branch.clone());
    }
    let cwd = resolve_cwd(ctx.cwd, ctx.worktree);
    current_branch(cwd)
}

pub(in crate::cli::hook) fn branch_label_from_pane(pane: &str) -> Option<String> {
    let wt_branch = tmux::get_pane_option_value(pane, "@pane_worktree_branch");
    if !wt_branch.is_empty() {
        return Some(wt_branch);
    }
    let cwd = tmux::get_pane_option_value(pane, "@pane_cwd");
    if cwd.is_empty() {
        None
    } else {
        current_branch(&cwd)
    }
}

pub(in crate::cli::hook) fn current_branch(path: &str) -> Option<String> {
    crate::git::run_git(path, &["rev-parse", "--abbrev-ref", "HEAD"])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "HEAD")
}

pub(in crate::cli::hook) fn repo_label_from_path(path: &str) -> Option<String> {
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let label = trimmed.rsplit('/').next().unwrap_or(trimmed).trim();
    if label.is_empty() {
        None
    } else {
        Some(label.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::WorktreeInfo;

    #[test]
    fn repo_label_from_path_strips_trailing_slash() {
        assert_eq!(
            repo_label_from_path("/home/user/repo/"),
            Some("repo".into())
        );
        assert_eq!(repo_label_from_path("/home/user/repo"), Some("repo".into()));
    }

    #[test]
    fn repo_label_from_path_rejects_empty_or_root() {
        assert!(repo_label_from_path("").is_none());
        assert!(repo_label_from_path("/").is_none());
    }

    #[test]
    fn repo_label_from_ctx_prefers_worktree_original_repo_dir() {
        let wt = Some(WorktreeInfo {
            name: "feat".into(),
            path: "/tmp/wt".into(),
            branch: "feat".into(),
            original_repo_dir: "/home/user/repo".into(),
        });
        let session_id = None;
        let ctx = AgentContext {
            agent: "claude",
            cwd: "/tmp/wt/src",
            permission_mode: "default",
            worktree: &wt,
            session_id: &session_id,
        };
        assert_eq!(repo_label_from_ctx(&ctx), Some("repo".into()));
    }

    #[test]
    fn repo_label_from_pane_prefers_pane_cwd_then_worktree_name() {
        let _guard = tmux::test_mock::install();
        let pane = "%PANE_REPO";
        tmux::test_mock::set(pane, "@pane_cwd", "/home/user/app");
        tmux::test_mock::set(pane, "@pane_worktree_name", "wt-name");

        assert_eq!(repo_label_from_pane(pane), Some("app".into()));

        tmux::test_mock::set(pane, "@pane_cwd", "");
        assert_eq!(repo_label_from_pane(pane), Some("wt-name".into()));
    }

    #[test]
    fn repo_label_from_pane_returns_none_when_all_empty() {
        let _guard = tmux::test_mock::install();
        let pane = "%EMPTY_REPO";
        assert!(repo_label_from_pane(pane).is_none());
    }

    #[test]
    fn branch_label_from_ctx_prefers_worktree_branch() {
        let wt = Some(WorktreeInfo {
            name: "feat".into(),
            path: "/tmp/wt".into(),
            branch: "feature/xyz".into(),
            original_repo_dir: "/home/user/repo".into(),
        });
        let session_id = None;
        let ctx = AgentContext {
            agent: "claude",
            cwd: "/tmp/wt/src",
            permission_mode: "default",
            worktree: &wt,
            session_id: &session_id,
        };
        assert_eq!(branch_label_from_ctx(&ctx), Some("feature/xyz".into()));
    }

    #[test]
    fn branch_label_from_pane_prefers_worktree_branch_option() {
        let _guard = tmux::test_mock::install();
        let pane = "%PANE_BRANCH";
        tmux::test_mock::set(pane, "@pane_worktree_branch", "feat/abc");
        tmux::test_mock::set(pane, "@pane_cwd", "/tmp/somewhere");
        assert_eq!(branch_label_from_pane(pane), Some("feat/abc".into()));
    }
}
