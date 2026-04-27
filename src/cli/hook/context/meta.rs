use super::location::{pane_writes_allowed, sync_pane_location};
use super::pending::{PENDING_SESSION_END, PENDING_WORKTREE_REMOVE};
use crate::event::WorktreeInfo;
use crate::tmux;

/// Bundle of hook-payload fields shared by 6 `AgentEvent` variants
/// (SessionStart / UserPromptSubmit / Notification / Stop / StopFailure /
/// PermissionDenied). Passing this as a single reference keeps each
/// variant handler's signature short and avoids `too_many_arguments`.
pub(in crate::cli::hook) struct AgentContext<'a> {
    pub(in crate::cli::hook) agent: &'a str,
    pub(in crate::cli::hook) cwd: &'a str,
    pub(in crate::cli::hook) permission_mode: &'a str,
    pub(in crate::cli::hook) worktree: &'a Option<WorktreeInfo>,
    pub(in crate::cli::hook) session_id: &'a Option<String>,
}

pub(in crate::cli::hook) fn make_ctx<'a>(
    agent: &'a str,
    cwd: &'a str,
    permission_mode: &'a str,
    worktree: &'a Option<WorktreeInfo>,
    session_id: &'a Option<String>,
) -> AgentContext<'a> {
    AgentContext {
        agent,
        cwd,
        permission_mode,
        worktree,
        session_id,
    }
}

pub(in crate::cli::hook) fn set_agent_meta(pane: &str, ctx: &AgentContext<'_>) {
    tmux::set_pane_option(pane, tmux::PANE_AGENT, ctx.agent);
    // `@pane_permission_mode` is parent-owned: a child agent can be in
    // a different mode (e.g. plan vs. default) and overwriting the
    // parent's value here would flip the badge mid-session. Gate the
    // write behind the same subagent guard as the cwd/worktree fields.
    if !ctx.permission_mode.is_empty() && pane_writes_allowed(pane) {
        tmux::set_pane_option(pane, tmux::PANE_PERMISSION_MODE, ctx.permission_mode);
    }
    sync_pane_location(pane, ctx.cwd, ctx.worktree, ctx.session_id);
}

pub(in crate::cli::hook) fn clear_run_state(pane: &str) {
    tmux::unset_pane_option(pane, tmux::PANE_STARTED_AT);
    tmux::unset_pane_option(pane, tmux::PANE_WAIT_REASON);
}

/// Check if a prompt is a system-injected message (not a real user prompt).
pub(in crate::cli::hook) fn is_system_message(s: &str) -> bool {
    s.contains("<task-notification>") || s.contains("<system-reminder>") || s.contains("<task-")
}

pub(in crate::cli::hook) fn clear_all_meta(pane: &str) {
    for key in &[
        tmux::PANE_AGENT,
        tmux::PANE_PROMPT,
        tmux::PANE_PROMPT_SOURCE,
        tmux::PANE_BG_CMD,
        tmux::PANE_SUBAGENTS,
        tmux::PANE_CWD,
        tmux::PANE_PERMISSION_MODE,
        tmux::PANE_WORKTREE_NAME,
        tmux::PANE_WORKTREE_BRANCH,
        tmux::PANE_SESSION_ID,
        PENDING_SESSION_END,
        PENDING_WORKTREE_REMOVE,
    ] {
        tmux::unset_pane_option(pane, key);
    }
    clear_run_state(pane);
}

/// Write a task-reset marker to the activity log so `parse_task_progress`
/// treats the upcoming run as a fresh batch — otherwise in-progress or
/// abandoned tasks from a previous run would accumulate into the next one.
///
/// Skipped while subagents are still active so a parent Stop event doesn't
/// wipe task state children are still driving.
pub(in crate::cli::hook) fn mark_task_reset(pane: &str) {
    if !pane_writes_allowed(pane) {
        return;
    }
    crate::cli::hook::activity::write_activity_entry(pane, crate::activity::TASK_RESET_MARKER, "");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn make_ctx_wires_all_fields() {
        let agent = "claude".to_string();
        let cwd = "/tmp".to_string();
        let pm = "auto".to_string();
        let worktree: Option<WorktreeInfo> = None;
        let sid: Option<String> = None;
        let ctx = make_ctx(&agent, &cwd, &pm, &worktree, &sid);
        assert_eq!(ctx.agent, "claude");
        assert_eq!(ctx.cwd, "/tmp");
        assert_eq!(ctx.permission_mode, "auto");
        assert!(ctx.worktree.is_none());
        assert!(ctx.session_id.is_none());
    }

    #[test]
    fn make_ctx_preserves_worktree_and_session_id() {
        let agent = "codex".to_string();
        let cwd = "/src".to_string();
        let pm = "plan".to_string();
        let worktree = Some(WorktreeInfo {
            name: "feat".into(),
            path: "/tmp/wt".into(),
            branch: "feature/x".into(),
            original_repo_dir: "/home/user/repo".into(),
        });
        let sid = Some("sess-abc".to_string());
        let ctx = make_ctx(&agent, &cwd, &pm, &worktree, &sid);
        assert_eq!(ctx.agent, "codex");
        assert_eq!(ctx.cwd, "/src");
        assert_eq!(ctx.permission_mode, "plan");
        assert_eq!(ctx.worktree.as_ref().map(|w| w.name.as_str()), Some("feat"));
        assert_eq!(ctx.session_id.as_deref(), Some("sess-abc"));
    }

    // ─── mark_task_reset ───────────────────────────────────────────

    #[test]
    fn mark_task_reset_writes_marker_when_no_subagents() {
        let _guard = tmux::test_mock::install();
        let pane_id = "%CLI_MARK_RESET";
        let path = crate::activity::log_file_path(pane_id);
        let _ = fs::remove_file(&path);

        mark_task_reset(pane_id);

        let content = fs::read_to_string(&path).unwrap();
        let marker = format!("|{}|", crate::activity::TASK_RESET_MARKER);
        assert!(content.contains(&marker), "marker not written: {content:?}");
        fs::remove_file(&path).ok();
    }

    #[test]
    fn mark_task_reset_skips_while_subagents_active() {
        let _guard = tmux::test_mock::install();
        let pane_id = "%CLI_MARK_RESET_SUBAGENT";
        tmux::test_mock::set(pane_id, tmux::PANE_SUBAGENTS, "Explore:abc");
        let path = crate::activity::log_file_path(pane_id);
        let _ = fs::remove_file(&path);

        mark_task_reset(pane_id);

        // No marker should be written because subagents are still active.
        assert!(!path.exists(), "log file created while subagents active");
    }

    // ─── is_system_message ────────────────────────────────────────

    #[test]
    fn system_message_task_notification() {
        assert!(is_system_message(
            "<task-notification><task-id>abc</task-id></task-notification>"
        ));
    }

    #[test]
    fn system_message_system_reminder() {
        assert!(is_system_message(
            "<system-reminder>some reminder</system-reminder>"
        ));
    }

    #[test]
    fn system_message_task_prefix() {
        assert!(is_system_message("<task-id>abc</task-id>"));
    }

    #[test]
    fn system_message_normal_prompt() {
        assert!(!is_system_message("fix the bug"));
    }

    #[test]
    fn system_message_empty() {
        assert!(!is_system_message(""));
    }

    #[test]
    fn system_message_mixed_content() {
        assert!(is_system_message(
            "hello <system-reminder>noise</system-reminder> world"
        ));
    }

    // ─── set_agent_meta ────────────────────────────────────────────

    #[test]
    fn set_agent_meta_does_not_clobber_parent_permission_mode_under_subagents() {
        let _guard = tmux::test_mock::install();
        let pane = "%PARENT_PERM";
        tmux::test_mock::set(pane, tmux::PANE_SUBAGENTS, "Explore:sub-1");
        tmux::test_mock::set(pane, tmux::PANE_PERMISSION_MODE, "plan");

        // A subagent fires a hook with `permission_mode: "default"` —
        // this must NOT flip the parent badge from "plan" back to
        // "default".
        let ctx = AgentContext {
            agent: "claude",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        };
        set_agent_meta(pane, &ctx);

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_PERMISSION_MODE).as_deref(),
            Some("plan"),
            "child hook must not overwrite parent's permission_mode"
        );
    }

    #[test]
    fn set_agent_meta_writes_permission_mode_when_no_subagents() {
        let _guard = tmux::test_mock::install();
        let pane = "%LONE_PERM";

        let ctx = AgentContext {
            agent: "claude",
            cwd: "/repo",
            permission_mode: "plan",
            worktree: &None,
            session_id: &None,
        };
        set_agent_meta(pane, &ctx);

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_PERMISSION_MODE).as_deref(),
            Some("plan"),
            "regular SessionStart should still write permission_mode"
        );
    }

    #[test]
    fn clear_run_state_removes_started_at_and_wait_reason() {
        let _guard = tmux::test_mock::install();
        let pane = "%CLEAR_RUN";
        tmux::test_mock::set(pane, tmux::PANE_STARTED_AT, "1700");
        tmux::test_mock::set(pane, tmux::PANE_WAIT_REASON, "permission");

        clear_run_state(pane);

        assert!(!tmux::test_mock::contains(pane, tmux::PANE_STARTED_AT));
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_WAIT_REASON));
    }

    #[test]
    fn clear_all_meta_drops_every_pane_option_we_own() {
        let _guard = tmux::test_mock::install();
        let pane = "%CLEAR_ALL";
        for key in [
            tmux::PANE_AGENT,
            tmux::PANE_PROMPT,
            tmux::PANE_PROMPT_SOURCE,
            tmux::PANE_SUBAGENTS,
            tmux::PANE_CWD,
            tmux::PANE_PERMISSION_MODE,
            tmux::PANE_WORKTREE_NAME,
            tmux::PANE_WORKTREE_BRANCH,
            tmux::PANE_SESSION_ID,
            tmux::PANE_STARTED_AT,
            tmux::PANE_WAIT_REASON,
            PENDING_SESSION_END,
            PENDING_WORKTREE_REMOVE,
        ] {
            tmux::test_mock::set(pane, key, "x");
        }

        clear_all_meta(pane);

        for key in [
            tmux::PANE_AGENT,
            tmux::PANE_PROMPT,
            tmux::PANE_PROMPT_SOURCE,
            tmux::PANE_SUBAGENTS,
            tmux::PANE_CWD,
            tmux::PANE_PERMISSION_MODE,
            tmux::PANE_WORKTREE_NAME,
            tmux::PANE_WORKTREE_BRANCH,
            tmux::PANE_SESSION_ID,
            tmux::PANE_STARTED_AT,
            tmux::PANE_WAIT_REASON,
            PENDING_SESSION_END,
            PENDING_WORKTREE_REMOVE,
        ] {
            assert!(
                !tmux::test_mock::contains(pane, key),
                "expected {key} cleared"
            );
        }
    }
}
