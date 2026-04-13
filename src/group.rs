use indexmap::IndexMap;

use crate::git::run_git;
use crate::tmux::PaneInfo;

/// Per-pane git metadata resolved from the pane's working directory.
#[derive(Debug, Clone, Default)]
pub struct PaneGitInfo {
    pub repo_root: Option<String>,
    pub branch: Option<String>,
    pub is_worktree: bool,
    pub worktree_name: Option<String>,
}

/// A group of panes working in the same repository (or directory).
#[derive(Debug, Clone)]
pub struct RepoGroup {
    /// Display name: repo directory basename, or raw path for non-git
    pub name: String,
    /// Whether any pane in the group belongs to the focused (active) window
    pub has_focus: bool,
    /// Panes in this group, with their git info
    pub panes: Vec<(PaneInfo, PaneGitInfo)>,
}

/// Resolve git info for a single pane path.
pub fn resolve_pane_git_info(path: &str) -> PaneGitInfo {
    if path.is_empty() {
        return PaneGitInfo::default();
    }

    // Single git call for all three values (one line per arg)
    let combined = run_git(
        path,
        &[
            "rev-parse",
            "--abbrev-ref",
            "HEAD",
            "--git-common-dir",
            "--git-dir",
        ],
    );
    let (branch, git_common_dir, git_dir) = match combined {
        Some(output) => {
            let mut lines = output.lines();
            let b = lines.next().map(|s| s.to_string());
            let c = lines.next().map(|s| s.to_string());
            let d = lines.next().map(|s| s.to_string());
            (b, c, d)
        }
        None => (None, None, None),
    };

    let is_worktree = match (&git_common_dir, &git_dir) {
        (Some(common), Some(dir)) => {
            let common_path = resolve_git_path(path, common);
            let dir_path = resolve_git_path(path, dir);
            common_path != dir_path
        }
        _ => false,
    };

    // --git-common-dir returns the .git dir of the main worktree;
    // its parent is the repo root, so worktrees share the same group key.
    let repo_root = git_common_dir
        .as_ref()
        .and_then(|common| {
            let abs = resolve_git_path(path, common);
            abs.parent().map(|p| p.to_string_lossy().to_string())
        })
        .or_else(|| run_git(path, &["rev-parse", "--show-toplevel"]));

    PaneGitInfo {
        repo_root,
        branch,
        is_worktree,
        worktree_name: None,
    }
}

/// Group all panes across all sessions by repo root.
/// Returns groups sorted alphabetically by display name (case-insensitive),
/// so the order is stable regardless of which pane is encountered first.
pub fn group_panes_by_repo(sessions: &[crate::tmux::SessionInfo]) -> Vec<RepoGroup> {
    let mut groups: IndexMap<String, RepoGroup> = IndexMap::new();
    let mut git_cache: std::collections::HashMap<String, PaneGitInfo> =
        std::collections::HashMap::new();

    for session in sessions {
        for window in &session.windows {
            for pane in &window.panes {
                let mut git_info = git_cache
                    .entry(pane.path.clone())
                    .or_insert_with(|| resolve_pane_git_info(&pane.path))
                    .clone();

                // Override with hook-provided worktree info (Claude Code provides this;
                // Codex does not, so git-command detection remains as fallback)
                if !pane.worktree_name.is_empty() {
                    git_info.worktree_name = Some(pane.worktree_name.clone());
                    git_info.is_worktree = true;
                }
                if !pane.worktree_branch.is_empty() {
                    git_info.branch = Some(pane.worktree_branch.clone());
                    git_info.is_worktree = true;
                }

                let group_key = git_info
                    .repo_root
                    .clone()
                    .unwrap_or_else(|| pane.path.clone());

                let display_name = group_key
                    .rsplit('/')
                    .next()
                    .unwrap_or(&group_key)
                    .to_string();

                let has_focus = window.window_active && pane.pane_active;

                let group = groups.entry(group_key).or_insert_with(|| RepoGroup {
                    name: display_name,
                    has_focus: false,
                    panes: Vec::new(),
                });

                if has_focus {
                    group.has_focus = true;
                }

                group.panes.push((pane.clone(), git_info));
            }
        }
    }

    let mut result: Vec<RepoGroup> = groups.into_values().collect();
    result.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    result
}

/// Resolve a possibly-relative git path to an absolute canonical path.
fn resolve_git_path(base: &str, git_path: &str) -> std::path::PathBuf {
    let p = if std::path::Path::new(git_path).is_absolute() {
        std::path::PathBuf::from(git_path)
    } else {
        std::path::PathBuf::from(base).join(git_path)
    };
    p.canonicalize().unwrap_or(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_git_info_returns_none_for_empty_path() {
        let info = resolve_pane_git_info("");
        assert!(info.branch.is_none());
        assert!(!info.is_worktree);
        assert!(info.repo_root.is_none());
    }

    #[test]
    fn resolve_git_info_for_real_repo() {
        // This test runs in the actual repo, so git commands work
        let info = resolve_pane_git_info(env!("CARGO_MANIFEST_DIR"));
        assert!(info.repo_root.is_some(), "should detect git repo");
        assert!(info.branch.is_some(), "should detect branch");
        // The repo root should be the tmux-agent-sidebar repo
        let root = info.repo_root.unwrap();
        assert!(
            root.contains("tmux-agent-sidebar"),
            "repo root should be tmux-agent-sidebar: {}",
            root
        );
    }

    #[test]
    fn worktree_and_main_share_same_repo_root() {
        // Both main and worktree should resolve to the same repo_root
        // We can only test the main worktree here, but verify the logic is consistent
        let info = resolve_pane_git_info(env!("CARGO_MANIFEST_DIR"));
        assert!(
            !info.is_worktree,
            "main checkout should not be detected as worktree"
        );
        assert!(info.repo_root.is_some());
    }

    // ─── resolve_git_path tests ─────────────────────────────────────

    #[test]
    fn resolve_git_path_absolute() {
        let result = resolve_git_path("/base", "/absolute/path");
        assert_eq!(result, std::path::PathBuf::from("/absolute/path"));
    }

    #[test]
    fn resolve_git_path_relative() {
        let result = resolve_git_path("/base/dir", "relative");
        assert_eq!(result, std::path::PathBuf::from("/base/dir/relative"));
    }

    // ─── group_panes_by_repo tests ──────────────────────────────────

    fn test_pane(id: &str, path: &str) -> PaneInfo {
        PaneInfo {
            pane_id: id.into(),
            pane_active: false,
            status: crate::tmux::PaneStatus::Running,
            attention: false,
            agent: crate::tmux::AgentType::Claude,
            path: path.into(),
            current_command: String::new(),
            prompt: String::new(),
            prompt_is_response: false,
            started_at: None,
            wait_reason: String::new(),
            permission_mode: crate::tmux::PermissionMode::Default,
            subagents: vec![],
            pane_pid: None,
            worktree_name: String::new(),
            worktree_branch: String::new(),
        }
    }

    fn test_window(panes: Vec<PaneInfo>, active: bool) -> crate::tmux::WindowInfo {
        crate::tmux::WindowInfo {
            window_id: "@0".into(),
            window_name: "test".into(),
            window_active: active,
            auto_rename: false,
            panes,
        }
    }

    fn test_session(windows: Vec<crate::tmux::WindowInfo>) -> crate::tmux::SessionInfo {
        crate::tmux::SessionInfo {
            session_name: "main".into(),
            windows,
        }
    }

    #[test]
    fn group_panes_empty_sessions() {
        let groups = group_panes_by_repo(&[]);
        assert!(groups.is_empty());
    }

    #[test]
    fn group_panes_same_repo() {
        // Two panes in the same real repo should be grouped together
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let pane1 = test_pane("%1", manifest_dir);
        let pane2 = test_pane("%2", manifest_dir);

        let sessions = vec![test_session(vec![test_window(vec![pane1, pane2], true)])];
        let groups = group_panes_by_repo(&sessions);

        assert_eq!(groups.len(), 1, "same repo path should produce one group");
        assert_eq!(groups[0].panes.len(), 2);
        assert_eq!(groups[0].panes[0].0.pane_id, "%1");
        assert_eq!(groups[0].panes[1].0.pane_id, "%2");
    }

    #[test]
    fn group_panes_non_git_path_uses_raw_path() {
        // A non-git path should use the raw path as the group key
        let pane = test_pane("%1", "/tmp/no-git-here");

        let sessions = vec![test_session(vec![test_window(vec![pane], true)])];
        let groups = group_panes_by_repo(&sessions);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "no-git-here");
    }

    #[test]
    fn group_panes_display_name_is_basename() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let pane = test_pane("%1", manifest_dir);

        let sessions = vec![test_session(vec![test_window(vec![pane], true)])];
        let groups = group_panes_by_repo(&sessions);

        assert_eq!(groups.len(), 1);
        assert_eq!(
            groups[0].name, "tmux-agent-sidebar",
            "display name should be repo basename"
        );
    }

    #[test]
    fn group_panes_has_focus_from_active_window_and_pane() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mut pane = test_pane("%1", manifest_dir);
        pane.pane_active = true;

        let sessions = vec![test_session(vec![test_window(vec![pane], true)])];
        let groups = group_panes_by_repo(&sessions);

        assert!(
            groups[0].has_focus,
            "active pane in active window should set has_focus"
        );
    }

    #[test]
    fn group_panes_no_focus_when_window_inactive() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mut pane = test_pane("%1", manifest_dir);
        pane.pane_active = true;

        let sessions = vec![test_session(vec![test_window(vec![pane], false)])]; // window_active=false
        let groups = group_panes_by_repo(&sessions);

        assert!(
            !groups[0].has_focus,
            "active pane in inactive window should not set has_focus"
        );
    }

    #[test]
    fn group_panes_empty_path_pane() {
        let pane = test_pane("%1", "");

        let sessions = vec![test_session(vec![test_window(vec![pane], true)])];
        let groups = group_panes_by_repo(&sessions);

        // Empty path pane should still be grouped (by empty key)
        assert_eq!(groups.len(), 1);
    }

    #[test]
    fn group_panes_multiple_sessions() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let pane1 = test_pane("%1", manifest_dir);
        let pane2 = test_pane("%2", "/tmp/other-project");

        let sessions = vec![
            test_session(vec![test_window(vec![pane1], true)]),
            crate::tmux::SessionInfo {
                session_name: "other".into(),
                windows: vec![test_window(vec![pane2], false)],
            },
        ];
        let groups = group_panes_by_repo(&sessions);

        assert_eq!(
            groups.len(),
            2,
            "different repos across sessions should produce separate groups"
        );
    }

    #[test]
    fn group_panes_sorted_by_name_case_insensitive() {
        // Groups should be sorted alphabetically regardless of encounter order
        let pane1 = test_pane("%1", "/tmp/zzz");
        let pane2 = test_pane("%2", "/tmp/Aaa");
        let pane3 = test_pane("%3", "/tmp/mmm");
        let pane4 = test_pane("%4", "/tmp/zzz");

        let sessions = vec![test_session(vec![test_window(
            vec![pane1, pane2, pane3, pane4],
            true,
        )])];
        let groups = group_panes_by_repo(&sessions);

        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].name, "Aaa");
        assert_eq!(groups[1].name, "mmm");
        assert_eq!(groups[2].name, "zzz");
        assert_eq!(groups[2].panes.len(), 2, "zzz should have 2 panes");
    }
}
