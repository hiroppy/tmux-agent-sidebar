use crate::event::WorktreeInfo;
use crate::tmux;

/// Returns whether the pane's cwd should be updated.
/// When subagents are active, events may come from a subagent running in a
/// worktree, so we should NOT overwrite the parent agent's cwd.
pub(super) fn should_update_cwd(current_subagents: &str) -> bool {
    current_subagents.is_empty()
}

/// Resolve the effective cwd for pane metadata.
/// When a worktree is active, prefer `original_repo_dir` so the sidebar
/// groups the pane under the original repository, not the worktree path.
pub(super) fn resolve_cwd<'a>(raw_cwd: &'a str, worktree: &'a Option<WorktreeInfo>) -> &'a str {
    if let Some(wt) = worktree
        && !wt.original_repo_dir.is_empty()
    {
        return &wt.original_repo_dir;
    }
    raw_cwd
}

/// Sync worktree name/branch pane options from hook payload.
/// Clears both options when worktree is None.
pub(super) fn sync_worktree_meta(pane: &str, worktree: &Option<WorktreeInfo>) {
    if let Some(wt) = worktree {
        if !wt.name.is_empty() {
            tmux::set_pane_option(pane, "@pane_worktree_name", &wt.name);
        }
        if !wt.branch.is_empty() {
            tmux::set_pane_option(pane, "@pane_worktree_branch", &wt.branch);
        }
    } else {
        tmux::unset_pane_option(pane, "@pane_worktree_name");
        tmux::unset_pane_option(pane, "@pane_worktree_branch");
    }
}

pub(super) fn sync_pane_location(
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

/// Bundle of hook-payload fields shared by 6 `AgentEvent` variants
/// (SessionStart / UserPromptSubmit / Notification / Stop / StopFailure /
/// PermissionDenied). Passing this as a single reference keeps each
/// variant handler's signature short and avoids `too_many_arguments`.
pub(super) struct AgentContext<'a> {
    pub(super) agent: &'a str,
    pub(super) cwd: &'a str,
    pub(super) permission_mode: &'a str,
    pub(super) worktree: &'a Option<WorktreeInfo>,
    pub(super) session_id: &'a Option<String>,
}

pub(super) fn make_ctx<'a>(
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

/// Returns true if pane-scoped writes from this hook event are safe to
/// apply to the pane's metadata. False while subagents are active so a
/// child hook cannot clobber the parent pane's identity.
pub(super) fn pane_writes_allowed(pane: &str) -> bool {
    let current_subagents = tmux::get_pane_option_value(pane, "@pane_subagents");
    should_update_cwd(&current_subagents)
}

pub(super) fn set_agent_meta(pane: &str, ctx: &AgentContext<'_>) {
    tmux::set_pane_option(pane, "@pane_agent", ctx.agent);
    // `@pane_permission_mode` is parent-owned: a child agent can be in
    // a different mode (e.g. plan vs. default) and overwriting the
    // parent's value here would flip the badge mid-session. Gate the
    // write behind the same subagent guard as the cwd/worktree fields.
    if !ctx.permission_mode.is_empty() && pane_writes_allowed(pane) {
        tmux::set_pane_option(pane, "@pane_permission_mode", ctx.permission_mode);
    }
    sync_pane_location(pane, ctx.cwd, ctx.worktree, ctx.session_id);
}

pub(super) fn clear_run_state(pane: &str) {
    tmux::unset_pane_option(pane, "@pane_started_at");
    tmux::unset_pane_option(pane, "@pane_wait_reason");
}

/// Check if a prompt is a system-injected message (not a real user prompt).
pub(super) fn is_system_message(s: &str) -> bool {
    s.contains("<task-notification>") || s.contains("<system-reminder>") || s.contains("<task-")
}

pub(super) fn clear_all_meta(pane: &str) {
    for key in &[
        "@pane_agent",
        "@pane_prompt",
        "@pane_prompt_source",
        "@pane_subagents",
        "@pane_cwd",
        "@pane_permission_mode",
        "@pane_worktree_name",
        "@pane_worktree_branch",
        "@pane_session_id",
        PENDING_SESSION_END,
        PENDING_WORKTREE_REMOVE,
    ] {
        tmux::unset_pane_option(pane, key);
    }
    clear_run_state(pane);
}

/// Tmux pane option set when SessionEnd is deferred because subagents are
/// still active. Drained by `on_subagent_stop` once `@pane_subagents`
/// becomes empty.
pub(super) const PENDING_SESSION_END: &str = "@pane_pending_session_end";
/// Same idea for WorktreeRemove.
pub(super) const PENDING_WORKTREE_REMOVE: &str = "@pane_pending_worktree_remove";

pub(super) fn mark_pending(pane: &str, key: &str) {
    tmux::set_pane_option(pane, key, "1");
}

/// Run any deferred teardowns recorded by previous calls to
/// `on_session_end` / `on_worktree_remove`. Called from `on_subagent_stop`
/// after the subagent list drains to empty so the parent pane is finally
/// cleaned up instead of being stranded with stale metadata.
pub(super) fn drain_pending_teardowns(pane: &str) {
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
pub(super) fn run_session_end_teardown(pane: &str) {
    super::super::set_attention(pane, "clear");
    clear_all_meta(pane);
    super::super::set_status(pane, "clear");
    let log_path = crate::activity::log_file_path(pane);
    let _ = std::fs::remove_file(log_path);
}

/// Side-effect body of the WorktreeRemove teardown. Same pattern as
/// `run_session_end_teardown` — single source of truth for both the inline
/// and deferred paths.
pub(super) fn run_worktree_remove_teardown(pane: &str) {
    sync_worktree_meta(pane, &None);
    // Clear hook-set cwd so query_sessions() falls back to
    // pane_current_path, avoiding stale worktree path association.
    tmux::unset_pane_option(pane, "@pane_cwd");
}

/// Append an agent type to a comma-separated subagent list.
/// Append a subagent entry to the comma-separated `@pane_subagents` list.
///
/// Format: each entry is `agent_type:agent_id`. The id suffix lets
/// `remove_subagent` match the exact instance on stop, and also lets the
/// UI render a stable `#<id-prefix>` tag that does not shift when siblings
/// stop.
pub(super) fn append_subagent(current: &str, agent_type: &str, agent_id: &str) -> String {
    let entry = format!("{}:{}", agent_type, agent_id);
    if current.is_empty() {
        entry
    } else {
        format!("{},{}", current, entry)
    }
}

/// Remove the entry with the given `agent_id` from the comma-separated list.
/// Returns `None` if `agent_id` is not present, `Some(new_list)` otherwise
/// (empty string if the list becomes empty).
pub(super) fn remove_subagent(current: &str, agent_id: &str) -> Option<String> {
    if current.is_empty() || agent_id.is_empty() {
        return None;
    }
    let needle = format!(":{}", agent_id);
    let items: Vec<&str> = current.split(',').collect();
    let idx = items.iter().position(|entry| entry.ends_with(&needle))?;
    let filtered: Vec<&str> = items
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != idx)
        .map(|(_, s)| *s)
        .collect();
    Some(filtered.join(","))
}

pub(super) fn now_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub(super) fn now_epoch_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Write a task-reset marker to the activity log so `parse_task_progress`
/// treats the upcoming run as a fresh batch — otherwise in-progress or
/// abandoned tasks from a previous run would accumulate into the next one.
///
/// Skipped while subagents are still active so a parent Stop event doesn't
/// wipe task state children are still driving.
pub(super) fn mark_task_reset(pane: &str) {
    let current_subagents = tmux::get_pane_option_value(pane, "@pane_subagents");
    if !current_subagents.is_empty() {
        return;
    }
    super::activity::write_activity_entry(pane, crate::activity::TASK_RESET_MARKER, "");
}

pub(super) fn repo_label_from_ctx(ctx: &AgentContext<'_>) -> Option<String> {
    let cwd = resolve_cwd(ctx.cwd, ctx.worktree);
    repo_label_from_path(cwd)
}

pub(super) fn repo_label_from_pane(pane: &str) -> Option<String> {
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

pub(super) fn branch_label_from_ctx(ctx: &AgentContext<'_>) -> Option<String> {
    if let Some(wt) = ctx.worktree
        && !wt.branch.is_empty()
    {
        return Some(wt.branch.clone());
    }
    let cwd = resolve_cwd(ctx.cwd, ctx.worktree);
    current_branch(cwd)
}

pub(super) fn branch_label_from_pane(pane: &str) -> Option<String> {
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

pub(super) fn current_branch(path: &str) -> Option<String> {
    crate::git::run_git(path, &["rev-parse", "--abbrev-ref", "HEAD"])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "HEAD")
}

pub(super) fn repo_label_from_path(path: &str) -> Option<String> {
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
    use std::fs;

    // ─── resolve_cwd tests ─────────────────────────────────────────

    // ─── make_ctx tests ────────────────────────────────────────────

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

    #[test]
    fn resolve_cwd_prefers_worktree_original_repo_dir() {
        let wt = crate::event::WorktreeInfo {
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
        let wt = crate::event::WorktreeInfo {
            name: "feat".into(),
            path: "/tmp/wt".into(),
            branch: "feat".into(),
            original_repo_dir: "".into(),
        };
        assert_eq!(resolve_cwd("/tmp/wt/src", &Some(wt)), "/tmp/wt/src");
    }

    #[test]
    fn repo_label_from_ctx_prefers_worktree_original_repo_dir() {
        let wt = Some(crate::event::WorktreeInfo {
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

    // ─── append_subagent tests ──────────────────────────────────────

    #[test]
    fn append_subagent_to_empty() {
        assert_eq!(append_subagent("", "Explore", "sub-1"), "Explore:sub-1");
    }

    #[test]
    fn append_subagent_to_existing() {
        assert_eq!(
            append_subagent("Explore:sub-1", "Plan", "sub-2"),
            "Explore:sub-1,Plan:sub-2"
        );
    }

    #[test]
    fn append_subagent_same_type_parallel() {
        // Two Explore subagents running in parallel must be stored as
        // distinct entries — the ids disambiguate them.
        let list = append_subagent("Explore:sub-1", "Explore", "sub-2");
        assert_eq!(list, "Explore:sub-1,Explore:sub-2");
    }

    // ─── remove_subagent tests ──────────────────────────────────────

    #[test]
    fn remove_subagent_empty_list() {
        assert_eq!(remove_subagent("", "sub-1"), None);
    }

    #[test]
    fn remove_subagent_empty_id_is_noop() {
        assert_eq!(remove_subagent("Explore:sub-1", ""), None);
    }

    #[test]
    fn remove_subagent_id_not_found() {
        assert_eq!(remove_subagent("Explore:sub-1,Plan:sub-2", "sub-9"), None);
    }

    #[test]
    fn remove_subagent_single_item() {
        assert_eq!(remove_subagent("Explore:sub-1", "sub-1"), Some("".into()));
    }

    #[test]
    fn remove_subagent_first_item() {
        assert_eq!(
            remove_subagent("Explore:sub-1,Plan:sub-2", "sub-1"),
            Some("Plan:sub-2".into())
        );
    }

    #[test]
    fn remove_subagent_middle_item() {
        assert_eq!(
            remove_subagent("Explore:sub-1,Plan:sub-2,Bash:sub-3", "sub-2"),
            Some("Explore:sub-1,Bash:sub-3".into())
        );
    }

    #[test]
    fn remove_subagent_last_item() {
        assert_eq!(
            remove_subagent("Explore:sub-1,Plan:sub-2", "sub-2"),
            Some("Explore:sub-1".into())
        );
    }

    #[test]
    fn remove_subagent_same_type_uses_id_not_position() {
        // Regression: with two Explore subagents running in parallel, stopping
        // the FIRST one (sub-1) must remove that specific entry, not the last
        // occurrence. Old type-based remove_last_subagent got this wrong.
        assert_eq!(
            remove_subagent("Explore:sub-1,Explore:sub-2", "sub-1"),
            Some("Explore:sub-2".into())
        );
    }

    #[test]
    fn remove_subagent_same_type_three_parallel() {
        // Stop the middle one of three same-type parallel subagents.
        assert_eq!(
            remove_subagent("Explore:a,Explore:b,Explore:c", "b"),
            Some("Explore:a,Explore:c".into())
        );
    }

    #[test]
    fn remove_subagent_ignores_id_collision_across_types() {
        // The `:id` match must include the colon prefix so a type name ending
        // with the id substring cannot match by accident.
        assert_eq!(
            remove_subagent("TrailingX:y,Explore:x", "x"),
            Some("TrailingX:y".into())
        );
    }

    // ─── mark_task_reset tests ──────────────────────────────────────

    #[test]
    fn mark_task_reset_writes_marker_when_no_subagents() {
        let _guard = crate::tmux::test_mock::install();
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
        let _guard = crate::tmux::test_mock::install();
        let pane_id = "%CLI_MARK_RESET_SUBAGENT";
        crate::tmux::test_mock::set(pane_id, "@pane_subagents", "Explore:abc");
        let path = crate::activity::log_file_path(pane_id);
        let _ = fs::remove_file(&path);

        mark_task_reset(pane_id);

        // No marker should be written because subagents are still active.
        assert!(!path.exists(), "log file created while subagents active");
    }

    // ─── is_system_message tests ────────────────────────────────────

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

    // ─── subagent lifecycle tests ───────────────────────────────────

    #[test]
    fn subagent_lifecycle_two_parallel_same_type_stop_first() {
        // Regression for the parallel-same-type bug. Two Explore subagents
        // start, then the FIRST one (sub-1) completes — id-based removal
        // must leave sub-2 in place.
        let list = append_subagent("", "Explore", "sub-1");
        let list = append_subagent(&list, "Explore", "sub-2");
        assert_eq!(list, "Explore:sub-1,Explore:sub-2");

        let remaining = remove_subagent(&list, "sub-1").unwrap();
        assert_eq!(remaining, "Explore:sub-2");

        let remaining = remove_subagent(&remaining, "sub-2").unwrap();
        assert_eq!(remaining, "");
    }

    #[test]
    fn subagent_lifecycle_mixed_types() {
        let list = append_subagent("", "Explore", "sub-1");
        let list = append_subagent(&list, "Plan", "sub-2");
        assert_eq!(list, "Explore:sub-1,Plan:sub-2");

        // Plan completes, Explore still running
        let remaining = remove_subagent(&list, "sub-2").unwrap();
        assert_eq!(remaining, "Explore:sub-1");
    }

    #[test]
    fn subagent_lifecycle_stop_unknown_id_is_noop() {
        // A stop with an unknown id should leave the list untouched.
        let list = append_subagent("", "Explore", "sub-1");
        assert_eq!(remove_subagent(&list, "sub-999"), None);
    }

    // ─── should_update_cwd tests (worktree subagent bug) ───────────

    #[test]
    fn should_update_cwd_when_no_subagents() {
        // No subagents active → safe to update cwd
        assert!(should_update_cwd(""));
    }

    #[test]
    fn should_not_update_cwd_when_subagent_active() {
        // Subagent is running (possibly in a worktree) → do NOT overwrite
        // parent's cwd, because the event may come from the subagent
        // which inherits the same $TMUX_PANE.
        assert!(!should_update_cwd("Explore:sub-1"));
    }

    #[test]
    fn should_not_update_cwd_when_multiple_subagents_active() {
        assert!(!should_update_cwd("Explore:sub-1,Plan:sub-2"));
    }

    #[test]
    fn should_update_cwd_lifecycle_subagent_start_then_stop() {
        // Full lifecycle: subagent starts → blocks cwd update → subagent stops → allows again
        let no_subagents = "";
        let one_subagent = append_subagent(no_subagents, "Explore", "sub-1");

        // Before subagent: cwd update allowed
        assert!(should_update_cwd(no_subagents));

        // During subagent: cwd update blocked
        assert!(!should_update_cwd(&one_subagent));

        // After subagent stops: cwd update allowed again
        let after_stop = remove_subagent(&one_subagent, "sub-1").unwrap();
        assert!(should_update_cwd(&after_stop));
    }

    #[test]
    fn should_update_cwd_nested_subagents_require_all_stopped() {
        // Two subagents running: cwd blocked until BOTH stop
        let list = append_subagent("", "Explore", "sub-1");
        let list = append_subagent(&list, "Plan", "sub-2");
        assert!(!should_update_cwd(&list));

        // One stops: still blocked
        let list = remove_subagent(&list, "sub-2").unwrap();
        assert!(!should_update_cwd(&list));

        // Both stopped: allowed
        let list = remove_subagent(&list, "sub-1").unwrap();
        assert!(should_update_cwd(&list));
    }

    #[test]
    fn should_update_cwd_race_condition_session_start_before_subagent_start() {
        // Edge case: if subagent's session-start fires BEFORE the parent's
        // subagent-start hook sets @pane_subagents, the cwd would be updated.
        // This documents the known limitation — @pane_subagents is still empty.
        let before_subagent_start_hook = "";
        assert!(
            should_update_cwd(before_subagent_start_hook),
            "known limitation: if session-start races ahead of subagent-start, cwd is updated"
        );
    }

    // ─── parent-pane preservation regression tests ──────────────────
    //
    // These tests use the `tmux::test_mock` thread-local store to
    // capture pane-option writes without shelling out to real tmux. They
    // pin the invariant that subagent-emitted hook events must not
    // overwrite or erase the parent pane's metadata.

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

        // Every parent pane-option must be untouched.
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

    // ─── permission_mode parent-protection regression tests ─────────

    #[test]
    fn set_agent_meta_does_not_clobber_parent_permission_mode_under_subagents() {
        let _guard = tmux::test_mock::install();
        let pane = "%PARENT_PERM";
        tmux::test_mock::set(pane, "@pane_subagents", "Explore:sub-1");
        tmux::test_mock::set(pane, "@pane_permission_mode", "plan");

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
            tmux::test_mock::get(pane, "@pane_permission_mode").as_deref(),
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
            tmux::test_mock::get(pane, "@pane_permission_mode").as_deref(),
            Some("plan"),
            "regular SessionStart should still write permission_mode"
        );
    }
}
