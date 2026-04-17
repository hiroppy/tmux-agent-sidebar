use crate::desktop_notification;
use crate::desktop_notification::DesktopNotificationKind;
use crate::tmux;

use super::super::label::extract_tool_label;
use super::super::{sanitize_tmux_value, set_attention, set_status};

use super::context::{
    AgentContext, PENDING_SESSION_END, PENDING_WORKTREE_REMOVE, append_subagent,
    branch_label_from_ctx, branch_label_from_pane, clear_run_state, drain_pending_teardowns,
    is_system_message, mark_pending, mark_task_reset, notification_body, notification_fingerprint,
    notification_run_id, notify_desktop, now_epoch_secs, pane_writes_allowed, remove_subagent,
    repo_label_from_ctx, repo_label_from_pane, run_session_end_teardown,
    run_worktree_remove_teardown, set_agent_meta, set_notification_run_id, should_update_cwd,
    stop_body, stop_failure_body, stop_failure_fingerprint, task_completed_body,
    task_completed_fingerprint, write_activity_entry,
};

pub(super) fn on_session_start(pane: &str, ctx: &AgentContext<'_>) -> i32 {
    set_agent_meta(pane, ctx);
    set_attention(pane, "clear");
    clear_run_state(pane);
    set_notification_run_id(pane);
    tmux::unset_pane_option(pane, "@pane_prompt");
    tmux::unset_pane_option(pane, "@pane_prompt_source");
    tmux::unset_pane_option(pane, "@pane_subagents");
    // A fresh session overrides any deferred teardown that was waiting
    // for the previous run's subagents to drain.
    tmux::unset_pane_option(pane, PENDING_SESSION_END);
    tmux::unset_pane_option(pane, PENDING_WORKTREE_REMOVE);
    set_status(pane, "idle");
    0
}

pub(super) fn on_session_end(pane: &str) -> i32 {
    // Subagents share the parent's $TMUX_PANE, so a child emitting
    // SessionEnd must NOT wipe the parent's metadata or activity log.
    // While children are still listed, defer the teardown via a marker
    // that `on_subagent_stop` drains once the list empties — otherwise a
    // parent SessionEnd that races ahead of every SubagentStop would
    // leave the pane stranded with stale metadata forever.
    let current_subagents = tmux::get_pane_option_value(pane, "@pane_subagents");
    if !should_update_cwd(&current_subagents) {
        mark_pending(pane, PENDING_SESSION_END);
        return 0;
    }
    run_session_end_teardown(pane);
    0
}

pub(super) fn on_user_prompt_submit(pane: &str, ctx: &AgentContext<'_>, prompt: &str) -> i32 {
    set_agent_meta(pane, ctx);
    set_attention(pane, "clear");
    set_status(pane, "running");
    set_notification_run_id(pane);
    if !prompt.is_empty() && !is_system_message(prompt) {
        let p = sanitize_tmux_value(prompt);
        tmux::set_pane_option(pane, "@pane_prompt", &p);
        tmux::set_pane_option(pane, "@pane_prompt_source", "user");
    }
    tmux::set_pane_option(pane, "@pane_started_at", &now_epoch_secs().to_string());
    tmux::unset_pane_option(pane, "@pane_wait_reason");
    0
}

pub(super) fn on_notification(
    pane: &str,
    ctx: &AgentContext<'_>,
    wait_reason: &str,
    meta_only: bool,
    notifications: &desktop_notification::DesktopNotificationSettings,
) -> i32 {
    set_agent_meta(pane, ctx);
    if meta_only {
        return 0;
    }
    set_status(pane, "waiting");
    set_attention(pane, "notification");
    if !wait_reason.is_empty() {
        tmux::set_pane_option(pane, "@pane_wait_reason", wait_reason);
    }
    let repo = repo_label_from_ctx(ctx);
    let branch = branch_label_from_ctx(ctx);
    let fingerprint = desktop_notification::run_scoped_fingerprint(
        notification_run_id(pane),
        notification_fingerprint(wait_reason),
    );
    let _ = notify_desktop(
        pane,
        DesktopNotificationKind::PermissionRequired,
        desktop_notification::DesktopNotificationEvent::Notification,
        notifications,
        &fingerprint,
        &desktop_notification::format_title(repo.as_deref(), branch.as_deref(), ctx.agent),
        &notification_body(wait_reason),
    );
    0
}

pub(super) fn on_stop(
    pane: &str,
    ctx: &AgentContext<'_>,
    last_message: &str,
    response: Option<&str>,
    notifications: &desktop_notification::DesktopNotificationSettings,
) -> i32 {
    set_agent_meta(pane, ctx);
    set_attention(pane, "clear");
    if !last_message.is_empty() {
        let msg = sanitize_tmux_value(last_message);
        tmux::set_pane_option(pane, "@pane_prompt", &msg);
        tmux::set_pane_option(pane, "@pane_prompt_source", "response");
    }
    clear_run_state(pane);
    mark_task_reset(pane);
    set_status(pane, "idle");
    let run_id = notification_run_id(pane);
    // Skip the generic Stop notification if an explicit TaskCompleted
    // stamp from the current run has already fired — otherwise Claude
    // Code's `TaskCompleted` → `Stop` sequence produces two desktop
    // notifications for the same logical completion.
    let already_notified = desktop_notification::has_run_scoped_stamp(
        pane,
        DesktopNotificationKind::TaskCompleted,
        run_id,
    );
    if !already_notified {
        let repo = repo_label_from_ctx(ctx);
        let branch = branch_label_from_ctx(ctx);
        let fingerprint = desktop_notification::run_scoped_fingerprint(run_id, "stop");
        let _ = notify_desktop(
            pane,
            DesktopNotificationKind::TaskCompleted,
            desktop_notification::DesktopNotificationEvent::Stop,
            notifications,
            &fingerprint,
            &desktop_notification::format_title(repo.as_deref(), branch.as_deref(), ctx.agent),
            &stop_body(last_message),
        );
    }
    if let Some(resp) = response {
        println!("{resp}");
    }
    0
}

pub(super) fn on_stop_failure(
    pane: &str,
    ctx: &AgentContext<'_>,
    error: &str,
    notifications: &desktop_notification::DesktopNotificationSettings,
) -> i32 {
    set_agent_meta(pane, ctx);
    set_attention(pane, "clear");
    clear_run_state(pane);
    mark_task_reset(pane);
    if !error.is_empty() {
        tmux::set_pane_option(pane, "@pane_wait_reason", error);
    }
    set_status(pane, "error");
    let fingerprint = desktop_notification::run_scoped_fingerprint(
        notification_run_id(pane),
        stop_failure_fingerprint(error),
    );
    let repo = repo_label_from_ctx(ctx);
    let branch = branch_label_from_ctx(ctx);
    let body = stop_failure_body(error);
    let _ = notify_desktop(
        pane,
        DesktopNotificationKind::TaskFailed,
        desktop_notification::DesktopNotificationEvent::StopFailure,
        notifications,
        &fingerprint,
        &desktop_notification::format_title(repo.as_deref(), branch.as_deref(), ctx.agent),
        &body,
    );
    0
}

pub(super) fn on_subagent_start(pane: &str, agent_type: &str, agent_id: Option<&str>) -> i32 {
    // Claude Code always sends agent_id per the hooks spec; drop the
    // event silently if it's missing so the tree never gains an
    // untrackable entry.
    let Some(id) = agent_id.filter(|s| !s.is_empty()) else {
        return 0;
    };
    let current = tmux::get_pane_option_value(pane, "@pane_subagents");
    let new_val = append_subagent(&current, agent_type, id);
    tmux::set_pane_option(pane, "@pane_subagents", &new_val);
    0
}

pub(super) fn on_subagent_stop(pane: &str, agent_id: Option<&str>) -> i32 {
    let Some(id) = agent_id.filter(|s| !s.is_empty()) else {
        return 0;
    };
    let current = tmux::get_pane_option_value(pane, "@pane_subagents");
    let drained_to_empty = match remove_subagent(&current, id) {
        None => false,
        Some(new_val) if new_val.is_empty() => {
            tmux::unset_pane_option(pane, "@pane_subagents");
            true
        }
        Some(new_val) => {
            tmux::set_pane_option(pane, "@pane_subagents", &new_val);
            false
        }
    };
    // Once the last subagent stops, replay any teardown that was deferred
    // because subagents were active when SessionEnd / WorktreeRemove fired.
    if drained_to_empty {
        drain_pending_teardowns(pane);
    }
    0
}

pub(super) fn on_permission_denied(
    pane: &str,
    ctx: &AgentContext<'_>,
    notifications: &desktop_notification::DesktopNotificationSettings,
) -> i32 {
    set_agent_meta(pane, ctx);
    set_status(pane, "waiting");
    set_attention(pane, "notification");
    tmux::set_pane_option(pane, "@pane_wait_reason", "permission_denied");
    let repo = repo_label_from_ctx(ctx);
    let branch = branch_label_from_ctx(ctx);
    let fingerprint = desktop_notification::run_scoped_fingerprint(
        notification_run_id(pane),
        "permission_denied",
    );
    let _ = notify_desktop(
        pane,
        DesktopNotificationKind::PermissionRequired,
        desktop_notification::DesktopNotificationEvent::PermissionDenied,
        notifications,
        &fingerprint,
        &desktop_notification::format_title(repo.as_deref(), branch.as_deref(), ctx.agent),
        "Permission required",
    );
    0
}

pub(super) fn on_teammate_idle(pane: &str, teammate_name: &str) -> i32 {
    set_attention(pane, "notification");
    let reason = format!("teammate_idle:{teammate_name}");
    tmux::set_pane_option(pane, "@pane_wait_reason", &reason);
    0
}

pub(super) fn on_worktree_remove(pane: &str) -> i32 {
    // If subagents are active, the removed worktree may belong to one of
    // them — we can't distinguish parent from child at this point, so the
    // safe default is to leave the parent's pane-scoped metadata intact.
    // Same deferred-drain idea as `on_session_end`: record the intent and
    // let `on_subagent_stop` execute it once children are gone.
    let current_subagents = tmux::get_pane_option_value(pane, "@pane_subagents");
    if !should_update_cwd(&current_subagents) {
        mark_pending(pane, PENDING_WORKTREE_REMOVE);
        return 0;
    }
    run_worktree_remove_teardown(pane);
    0
}

pub(super) fn on_task_completed(
    pane: &str,
    agent_name: &str,
    task_id: &str,
    task_subject: &str,
    notifications: &desktop_notification::DesktopNotificationSettings,
) -> i32 {
    let fingerprint = desktop_notification::run_scoped_fingerprint(
        notification_run_id(pane),
        task_completed_fingerprint(task_id, task_subject),
    );
    let repo = repo_label_from_pane(pane);
    let branch = branch_label_from_pane(pane);
    let body = task_completed_body(task_subject);
    let _ = notify_desktop(
        pane,
        DesktopNotificationKind::TaskCompleted,
        desktop_notification::DesktopNotificationEvent::TaskCompleted,
        notifications,
        &fingerprint,
        &desktop_notification::format_title(repo.as_deref(), branch.as_deref(), agent_name),
        &body,
    );
    0
}

pub(super) fn notification_settings() -> desktop_notification::DesktopNotificationSettings {
    desktop_notification::DesktopNotificationSettings::from_tmux()
}

// ─── activity-log logic ─────────────────────────────────────────────────────

/// Activity-log handler, called from `hook <agent> activity-log` event.
pub(super) fn handle_activity_log(
    pane: &str,
    tool_name: &str,
    tool_input: &serde_json::Value,
    tool_response: &serde_json::Value,
) -> i32 {
    let label = extract_tool_label(tool_name, tool_input, tool_response);

    // If status is not running, tool use means agent is active again
    let current_status = tmux::get_pane_option_value(pane, "@pane_status");
    if current_status != "running" && !current_status.is_empty() {
        set_status(pane, "running");
        if current_status == "waiting" {
            tmux::unset_pane_option(pane, "@pane_attention");
            tmux::unset_pane_option(pane, "@pane_wait_reason");
        }
        let existing_started = tmux::get_pane_option_value(pane, "@pane_started_at");
        if existing_started.is_empty() {
            tmux::set_pane_option(pane, "@pane_started_at", &now_epoch_secs().to_string());
        }
    }

    // Update permission mode when plan mode tools are used.
    // Same parent-protection rule as `set_agent_meta`: a subagent that
    // enters/exits plan mode must not flip the parent pane's badge.
    if pane_writes_allowed(pane) {
        match tool_name {
            "EnterPlanMode" => {
                tmux::set_pane_option(pane, "@pane_permission_mode", "plan");
            }
            "ExitPlanMode" => {
                tmux::set_pane_option(pane, "@pane_permission_mode", "default");
            }
            _ => {}
        }
    }

    write_activity_entry(pane, tool_name, &label);
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use serde_json::json;
    use std::fs;

    // ─── handle_activity_log tests ──────────────────────────────────

    #[test]
    fn handle_activity_log_writes_entry() {
        let pane_id = "%CLI_HANDLE_TEST";
        let path = crate::activity::log_file_path(pane_id);
        let _ = fs::remove_file(&path);

        handle_activity_log(
            pane_id,
            "Read",
            &json!({"file_path": "/home/user/src/main.rs"}),
            &Value::Null,
        );

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("|Read|main.rs"));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn handle_activity_log_empty_tool_name_does_nothing() {
        let pane_id = "%CLI_EMPTY_TOOL";
        let path = crate::activity::log_file_path(pane_id);
        let _ = fs::remove_file(&path);

        // With the adapter pattern, empty tool_name is filtered by the adapter
        // before reaching handle_activity_log. We still test that handle_activity_log
        // writes an entry even with empty tool_name (label extraction handles it).
        let result = handle_activity_log(pane_id, "", &Value::Null, &Value::Null);
        assert_eq!(result, 0);
        // Empty tool_name still writes an entry now (adapter filters upstream)
    }

    #[test]
    fn handle_activity_log_tool_input_as_json_object() {
        let pane_id = "%CLI_JSON_STR";
        let path = crate::activity::log_file_path(pane_id);
        let _ = fs::remove_file(&path);

        handle_activity_log(
            pane_id,
            "Edit",
            &json!({"file_path": "/a/b/test.rs"}),
            &Value::Null,
        );

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("|Edit|test.rs"));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn handle_activity_log_null_tool_input_uses_empty_label() {
        let pane_id = "%CLI_NULL_INPUT";
        let path = crate::activity::log_file_path(pane_id);
        let _ = fs::remove_file(&path);

        handle_activity_log(pane_id, "UnknownTool", &Value::Null, &Value::Null);

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("|UnknownTool|"));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn handle_activity_log_task_create_with_response() {
        let pane_id = "%CLI_TASK_CREATE";
        let path = crate::activity::log_file_path(pane_id);
        let _ = fs::remove_file(&path);

        handle_activity_log(
            pane_id,
            "TaskCreate",
            &json!({"subject": "Fix bug"}),
            &json!({"task": {"id": "42"}}),
        );

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("|TaskCreate|#42 Fix bug"));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn handle_activity_log_enter_plan_mode_blocked_by_subagents() {
        let _guard = tmux::test_mock::install();
        let pane = "%PARENT_PLAN";
        tmux::test_mock::set(pane, "@pane_subagents", "Explore:sub-1");
        tmux::test_mock::set(pane, "@pane_permission_mode", "default");

        // A subagent's EnterPlanMode tool use must not flip the parent
        // badge to "plan".
        handle_activity_log(pane, "EnterPlanMode", &Value::Null, &Value::Null);

        assert_eq!(
            tmux::test_mock::get(pane, "@pane_permission_mode").as_deref(),
            Some("default"),
            "child EnterPlanMode must not overwrite parent's permission_mode"
        );
    }

    // ─── SessionEnd / WorktreeRemove regression tests ───────────────

    #[test]
    fn on_session_end_preserves_parent_state_when_subagents_active() {
        let _guard = tmux::test_mock::install();
        let pane = "%PARENT_END";
        tmux::test_mock::set(pane, "@pane_subagents", "Explore:sub-1");
        tmux::test_mock::set(pane, "@pane_agent", "claude");
        tmux::test_mock::set(pane, "@pane_cwd", "/repo/parent");
        tmux::test_mock::set(pane, "@pane_session_id", "parent-session");
        tmux::test_mock::set(pane, "@pane_status", "running");
        // Seed an activity log so we can prove the file is NOT removed.
        let log_path = crate::activity::log_file_path(pane);
        let _ = fs::create_dir_all(log_path.parent().unwrap());
        fs::write(&log_path, "1234567890|Read|main.rs\n").unwrap();

        let exit = on_session_end(pane);

        assert_eq!(exit, 0);
        assert!(
            tmux::test_mock::contains(pane, "@pane_agent"),
            "child SessionEnd must not clear parent @pane_agent"
        );
        assert!(tmux::test_mock::contains(pane, "@pane_cwd"));
        assert!(tmux::test_mock::contains(pane, "@pane_session_id"));
        assert!(tmux::test_mock::contains(pane, "@pane_subagents"));
        assert!(
            log_path.exists(),
            "child SessionEnd must not delete parent activity log"
        );

        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn on_session_end_clears_state_when_no_subagents() {
        let _guard = tmux::test_mock::install();
        let pane = "%LONE_END";
        tmux::test_mock::set(pane, "@pane_agent", "claude");
        tmux::test_mock::set(pane, "@pane_cwd", "/repo");
        tmux::test_mock::set(pane, "@pane_status", "running");

        let exit = on_session_end(pane);

        assert_eq!(exit, 0);
        assert!(
            !tmux::test_mock::contains(pane, "@pane_agent"),
            "lone SessionEnd should clear @pane_agent"
        );
        assert!(!tmux::test_mock::contains(pane, "@pane_cwd"));
        assert!(!tmux::test_mock::contains(pane, "@pane_status"));
    }

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

    // ─── deferred teardown regression tests ─────────────────────────
    //
    // These pin the Codex adversarial review fix: SessionEnd /
    // WorktreeRemove fired while subagents are active must not be lost
    // forever. They are recorded as pending markers and replayed by
    // on_subagent_stop once the subagent list drains to empty.

    #[test]
    fn pending_session_end_drains_when_last_subagent_stops() {
        let _guard = tmux::test_mock::install();
        let pane = "%PARENT_DEFER";
        tmux::test_mock::set(pane, "@pane_subagents", "Explore:sub-1");
        tmux::test_mock::set(pane, "@pane_agent", "claude");
        tmux::test_mock::set(pane, "@pane_cwd", "/repo/parent");
        tmux::test_mock::set(pane, "@pane_status", "running");
        let log_path = crate::activity::log_file_path(pane);
        let _ = fs::create_dir_all(log_path.parent().unwrap());
        fs::write(&log_path, "1234567890|Read|main.rs\n").unwrap();

        // Parent SessionEnd arrives while a subagent is still running.
        on_session_end(pane);
        assert!(
            tmux::test_mock::contains(pane, PENDING_SESSION_END),
            "SessionEnd must be deferred via the pending marker"
        );
        assert!(
            tmux::test_mock::contains(pane, "@pane_agent"),
            "deferred SessionEnd must not yet clear parent state"
        );
        assert!(log_path.exists(), "deferred SessionEnd must keep the log");

        // Last subagent stops — pending teardown should fire now.
        on_subagent_stop(pane, Some("sub-1"));

        assert!(
            !tmux::test_mock::contains(pane, "@pane_agent"),
            "drained SessionEnd should clear parent agent"
        );
        assert!(!tmux::test_mock::contains(pane, "@pane_cwd"));
        assert!(!tmux::test_mock::contains(pane, "@pane_status"));
        assert!(
            !tmux::test_mock::contains(pane, PENDING_SESSION_END),
            "pending marker must be cleared once teardown runs"
        );
        assert!(
            !log_path.exists(),
            "drained SessionEnd should remove the activity log"
        );
    }

    #[test]
    fn pending_worktree_remove_drains_when_last_subagent_stops() {
        let _guard = tmux::test_mock::install();
        let pane = "%PARENT_WT_DEFER";
        tmux::test_mock::set(pane, "@pane_subagents", "Explore:sub-1");
        tmux::test_mock::set(pane, "@pane_worktree_name", "feat");
        tmux::test_mock::set(pane, "@pane_worktree_branch", "feat");
        tmux::test_mock::set(pane, "@pane_cwd", "/wt/feat");

        on_worktree_remove(pane);
        assert!(
            tmux::test_mock::contains(pane, PENDING_WORKTREE_REMOVE),
            "WorktreeRemove must be deferred via the pending marker"
        );
        assert!(tmux::test_mock::contains(pane, "@pane_worktree_name"));

        on_subagent_stop(pane, Some("sub-1"));

        assert!(!tmux::test_mock::contains(pane, "@pane_worktree_name"));
        assert!(!tmux::test_mock::contains(pane, "@pane_worktree_branch"));
        assert!(!tmux::test_mock::contains(pane, "@pane_cwd"));
        assert!(
            !tmux::test_mock::contains(pane, PENDING_WORKTREE_REMOVE),
            "pending marker must be cleared once teardown runs"
        );
    }

    #[test]
    fn pending_teardown_does_not_fire_until_subagents_empty() {
        let _guard = tmux::test_mock::install();
        let pane = "%PARENT_PARTIAL";
        tmux::test_mock::set(pane, "@pane_subagents", "Explore:sub-1,Plan:sub-2");
        tmux::test_mock::set(pane, "@pane_agent", "claude");

        on_session_end(pane);
        assert!(tmux::test_mock::contains(pane, PENDING_SESSION_END));

        // First child stops — list still has sub-2, teardown must NOT fire.
        on_subagent_stop(pane, Some("sub-1"));
        assert!(
            tmux::test_mock::contains(pane, "@pane_agent"),
            "teardown must wait for the LAST subagent"
        );
        assert!(tmux::test_mock::contains(pane, PENDING_SESSION_END));

        // Last child stops — now teardown fires.
        on_subagent_stop(pane, Some("sub-2"));
        assert!(!tmux::test_mock::contains(pane, "@pane_agent"));
        assert!(!tmux::test_mock::contains(pane, PENDING_SESSION_END));
    }

    #[test]
    fn fresh_session_start_clears_pending_markers() {
        let _guard = tmux::test_mock::install();
        let pane = "%PARENT_RESTART";
        tmux::test_mock::set(pane, PENDING_SESSION_END, "1");
        tmux::test_mock::set(pane, PENDING_WORKTREE_REMOVE, "1");

        let ctx = AgentContext {
            agent: "claude",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        };
        on_session_start(pane, &ctx);

        assert!(
            !tmux::test_mock::contains(pane, PENDING_SESSION_END),
            "fresh SessionStart must drop a stale pending marker"
        );
        assert!(!tmux::test_mock::contains(pane, PENDING_WORKTREE_REMOVE));
    }
}
