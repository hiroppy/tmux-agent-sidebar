use crate::event::{AgentEvent, WorktreeInfo, resolve_adapter};
use crate::tmux;

use super::label::extract_tool_label;
use super::{
    local_time_hhmm, read_stdin_json, sanitize_tmux_value, set_attention, set_status, tmux_pane,
};

/// Returns whether the pane's cwd should be updated.
/// When subagents are active, events may come from a subagent running in a
/// worktree, so we should NOT overwrite the parent agent's cwd.
fn should_update_cwd(current_subagents: &str) -> bool {
    current_subagents.is_empty()
}

/// Resolve the effective cwd for pane metadata.
/// When a worktree is active, prefer `original_repo_dir` so the sidebar
/// groups the pane under the original repository, not the worktree path.
fn resolve_cwd<'a>(raw_cwd: &'a str, worktree: &'a Option<WorktreeInfo>) -> &'a str {
    if let Some(wt) = worktree
        && !wt.original_repo_dir.is_empty()
    {
        return &wt.original_repo_dir;
    }
    raw_cwd
}

/// Sync worktree name/branch pane options from hook payload.
/// Clears both options when worktree is None.
fn sync_worktree_meta(pane: &str, worktree: &Option<WorktreeInfo>) {
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

fn sync_pane_location(
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
struct AgentContext<'a> {
    agent: &'a str,
    cwd: &'a str,
    permission_mode: &'a str,
    worktree: &'a Option<WorktreeInfo>,
    session_id: &'a Option<String>,
}

/// Returns true if pane-scoped writes from this hook event are safe to
/// apply to the pane's metadata. False while subagents are active so a
/// child hook cannot clobber the parent pane's identity.
fn pane_writes_allowed(pane: &str) -> bool {
    let current_subagents = tmux::get_pane_option_value(pane, "@pane_subagents");
    should_update_cwd(&current_subagents)
}

fn set_agent_meta(pane: &str, ctx: &AgentContext<'_>) {
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

fn clear_run_state(pane: &str) {
    tmux::unset_pane_option(pane, "@pane_started_at");
    tmux::unset_pane_option(pane, "@pane_wait_reason");
}

/// Check if a prompt is a system-injected message (not a real user prompt).
fn is_system_message(s: &str) -> bool {
    s.contains("<task-notification>") || s.contains("<system-reminder>") || s.contains("<task-")
}

fn clear_all_meta(pane: &str) {
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
const PENDING_SESSION_END: &str = "@pane_pending_session_end";
/// Same idea for WorktreeRemove.
const PENDING_WORKTREE_REMOVE: &str = "@pane_pending_worktree_remove";

fn mark_pending(pane: &str, key: &str) {
    tmux::set_pane_option(pane, key, "1");
}

/// Run any deferred teardowns recorded by previous calls to
/// `on_session_end` / `on_worktree_remove`. Called from `on_subagent_stop`
/// after the subagent list drains to empty so the parent pane is finally
/// cleaned up instead of being stranded with stale metadata.
fn drain_pending_teardowns(pane: &str) {
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
fn run_session_end_teardown(pane: &str) {
    set_attention(pane, "clear");
    clear_all_meta(pane);
    set_status(pane, "clear");
    let log_path = crate::activity::log_file_path(pane);
    let _ = std::fs::remove_file(log_path);
}

/// Side-effect body of the WorktreeRemove teardown. Same pattern as
/// `run_session_end_teardown` — single source of truth for both the inline
/// and deferred paths.
fn run_worktree_remove_teardown(pane: &str) {
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
fn append_subagent(current: &str, agent_type: &str, agent_id: &str) -> String {
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
fn remove_subagent(current: &str, agent_id: &str) -> Option<String> {
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

/// Write a single activity entry to the log file and trim if needed.
fn write_activity_entry(pane: &str, tool_name: &str, label: &str) {
    let log_path = crate::activity::log_file_path(pane);
    let label = sanitize_tmux_value(label);
    let timestamp = local_time_hhmm();
    let line = format!("{}|{}|{}\n", timestamp, tool_name, label);

    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let _ = f.write_all(line.as_bytes());
    }

    trim_log_file(&log_path, 200, 210);
}

/// Trim a log file to `keep` lines when it exceeds `threshold` lines.
fn trim_log_file(path: &std::path::Path, keep: usize, threshold: usize) {
    if let Ok(content) = std::fs::read_to_string(path) {
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() > threshold {
            let start = lines.len() - keep;
            let _ = std::fs::write(path, lines[start..].join("\n") + "\n");
        }
    }
}

// ─── hook subcommand ────────────────────────────────────────────────────────

pub(crate) fn cmd_hook(args: &[String]) -> i32 {
    let agent_name = args.first().map(|s| s.as_str()).unwrap_or("");
    let event_name = args.get(1).map(|s| s.as_str()).unwrap_or("");

    if agent_name.is_empty() || event_name.is_empty() {
        return 0;
    }

    let Some(adapter) = resolve_adapter(agent_name) else {
        return 0;
    };

    let pane = tmux_pane();
    if pane.is_empty() {
        return 0;
    }

    let input = read_stdin_json();
    let Some(event) = adapter.parse(event_name, &input) else {
        return 0;
    };

    handle_event(&pane, event)
}

// ─── event handler ──────────────────────────────────────────────────────────

fn handle_event(pane: &str, event: AgentEvent) -> i32 {
    match event {
        AgentEvent::SessionStart {
            agent,
            cwd,
            permission_mode,
            worktree,
            session_id,
            ..
        } => on_session_start(
            pane,
            &AgentContext {
                agent: &agent,
                cwd: &cwd,
                permission_mode: &permission_mode,
                worktree: &worktree,
                session_id: &session_id,
            },
        ),
        AgentEvent::SessionEnd => on_session_end(pane),
        AgentEvent::UserPromptSubmit {
            agent,
            cwd,
            permission_mode,
            prompt,
            worktree,
            session_id,
            ..
        } => on_user_prompt_submit(
            pane,
            &AgentContext {
                agent: &agent,
                cwd: &cwd,
                permission_mode: &permission_mode,
                worktree: &worktree,
                session_id: &session_id,
            },
            &prompt,
        ),
        AgentEvent::Notification {
            agent,
            cwd,
            permission_mode,
            wait_reason,
            meta_only,
            worktree,
            session_id,
            ..
        } => on_notification(
            pane,
            &AgentContext {
                agent: &agent,
                cwd: &cwd,
                permission_mode: &permission_mode,
                worktree: &worktree,
                session_id: &session_id,
            },
            &wait_reason,
            meta_only,
        ),
        AgentEvent::Stop {
            agent,
            cwd,
            permission_mode,
            last_message,
            response,
            worktree,
            session_id,
            ..
        } => on_stop(
            pane,
            &AgentContext {
                agent: &agent,
                cwd: &cwd,
                permission_mode: &permission_mode,
                worktree: &worktree,
                session_id: &session_id,
            },
            &last_message,
            response.as_deref(),
        ),
        AgentEvent::StopFailure {
            agent,
            cwd,
            permission_mode,
            error,
            worktree,
            session_id,
            ..
        } => on_stop_failure(
            pane,
            &AgentContext {
                agent: &agent,
                cwd: &cwd,
                permission_mode: &permission_mode,
                worktree: &worktree,
                session_id: &session_id,
            },
            &error,
        ),
        AgentEvent::SubagentStart {
            agent_type,
            agent_id,
        } => on_subagent_start(pane, &agent_type, agent_id.as_deref()),
        AgentEvent::SubagentStop { agent_id, .. } => on_subagent_stop(pane, agent_id.as_deref()),
        AgentEvent::ActivityLog {
            tool_name,
            tool_input,
            tool_response,
        } => handle_activity_log(pane, &tool_name, &tool_input, &tool_response),
        AgentEvent::PermissionDenied {
            agent,
            cwd,
            permission_mode,
            worktree,
            session_id,
            ..
        } => on_permission_denied(
            pane,
            &AgentContext {
                agent: &agent,
                cwd: &cwd,
                permission_mode: &permission_mode,
                worktree: &worktree,
                session_id: &session_id,
            },
        ),
        AgentEvent::CwdChanged {
            cwd,
            worktree,
            session_id,
            ..
        } => {
            sync_pane_location(pane, &cwd, &worktree, &session_id);
            0
        }
        AgentEvent::TaskCreated { .. } => 0,
        AgentEvent::TaskCompleted { .. } => {
            set_attention(pane, "notification");
            0
        }
        AgentEvent::TeammateIdle { teammate_name, .. } => on_teammate_idle(pane, &teammate_name),
        AgentEvent::WorktreeCreate => 0,
        AgentEvent::WorktreeRemove { .. } => on_worktree_remove(pane),
    }
}

fn now_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn on_session_start(pane: &str, ctx: &AgentContext<'_>) -> i32 {
    set_agent_meta(pane, ctx);
    set_attention(pane, "clear");
    clear_run_state(pane);
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

fn on_session_end(pane: &str) -> i32 {
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

fn on_user_prompt_submit(pane: &str, ctx: &AgentContext<'_>, prompt: &str) -> i32 {
    set_agent_meta(pane, ctx);
    set_attention(pane, "clear");
    set_status(pane, "running");
    if !prompt.is_empty() && !is_system_message(prompt) {
        let p = sanitize_tmux_value(prompt);
        tmux::set_pane_option(pane, "@pane_prompt", &p);
        tmux::set_pane_option(pane, "@pane_prompt_source", "user");
    }
    tmux::set_pane_option(pane, "@pane_started_at", &now_epoch_secs().to_string());
    tmux::unset_pane_option(pane, "@pane_wait_reason");
    0
}

fn on_notification(pane: &str, ctx: &AgentContext<'_>, wait_reason: &str, meta_only: bool) -> i32 {
    set_agent_meta(pane, ctx);
    if meta_only {
        return 0;
    }
    set_status(pane, "waiting");
    set_attention(pane, "notification");
    if !wait_reason.is_empty() {
        tmux::set_pane_option(pane, "@pane_wait_reason", wait_reason);
    }
    0
}

fn on_stop(pane: &str, ctx: &AgentContext<'_>, last_message: &str, response: Option<&str>) -> i32 {
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
    if let Some(resp) = response {
        println!("{resp}");
    }
    0
}

fn on_stop_failure(pane: &str, ctx: &AgentContext<'_>, error: &str) -> i32 {
    set_agent_meta(pane, ctx);
    set_attention(pane, "clear");
    clear_run_state(pane);
    mark_task_reset(pane);
    if !error.is_empty() {
        tmux::set_pane_option(pane, "@pane_wait_reason", error);
    }
    set_status(pane, "error");
    0
}

/// Write a task-reset marker to the activity log so `parse_task_progress`
/// treats the upcoming run as a fresh batch — otherwise in-progress or
/// abandoned tasks from a previous run would accumulate into the next one.
///
/// Skipped while subagents are still active so a parent Stop event doesn't
/// wipe task state children are still driving.
fn mark_task_reset(pane: &str) {
    let current_subagents = tmux::get_pane_option_value(pane, "@pane_subagents");
    if !current_subagents.is_empty() {
        return;
    }
    write_activity_entry(pane, crate::activity::TASK_RESET_MARKER, "");
}

fn on_subagent_start(pane: &str, agent_type: &str, agent_id: Option<&str>) -> i32 {
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

fn on_subagent_stop(pane: &str, agent_id: Option<&str>) -> i32 {
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

fn on_permission_denied(pane: &str, ctx: &AgentContext<'_>) -> i32 {
    set_agent_meta(pane, ctx);
    set_status(pane, "waiting");
    set_attention(pane, "notification");
    tmux::set_pane_option(pane, "@pane_wait_reason", "permission_denied");
    0
}

fn on_teammate_idle(pane: &str, teammate_name: &str) -> i32 {
    set_attention(pane, "notification");
    let reason = format!("teammate_idle:{teammate_name}");
    tmux::set_pane_option(pane, "@pane_wait_reason", &reason);
    0
}

fn on_worktree_remove(pane: &str) -> i32 {
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

// ─── activity-log logic ─────────────────────────────────────────────────────

/// Activity-log handler, called from `hook <agent> activity-log` event.
fn handle_activity_log(
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

    // ─── resolve_cwd tests ─────────────────────────────────────────

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

    // ─── trim_log_file tests ────────────────────────────────────────

    #[test]
    fn trim_log_file_under_threshold_no_change() {
        let dir = std::env::temp_dir();
        let path = dir.join("trim_test_under.log");
        fs::write(&path, "line1\nline2\nline3\n").unwrap();

        trim_log_file(&path, 2, 5);

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().count(), 3);
        fs::remove_file(&path).ok();
    }

    #[test]
    fn trim_log_file_over_threshold_trims() {
        let dir = std::env::temp_dir();
        let path = dir.join("trim_test_over.log");
        let lines: Vec<String> = (1..=15).map(|i| format!("line{}", i)).collect();
        fs::write(&path, lines.join("\n") + "\n").unwrap();

        trim_log_file(&path, 5, 10);

        let content = fs::read_to_string(&path).unwrap();
        let remaining: Vec<&str> = content.lines().collect();
        assert_eq!(remaining.len(), 5);
        assert_eq!(remaining[0], "line11");
        assert_eq!(remaining[4], "line15");
        fs::remove_file(&path).ok();
    }

    #[test]
    fn trim_log_file_exactly_at_threshold_no_change() {
        let dir = std::env::temp_dir();
        let path = dir.join("trim_test_exact.log");
        let lines: Vec<String> = (1..=10).map(|i| format!("line{}", i)).collect();
        fs::write(&path, lines.join("\n") + "\n").unwrap();

        trim_log_file(&path, 5, 10);

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().count(), 10);
        fs::remove_file(&path).ok();
    }

    #[test]
    fn trim_log_file_nonexistent_file_no_panic() {
        let dir = std::env::temp_dir();
        let path = dir.join("trim_test_nonexistent.log");
        let _ = fs::remove_file(&path);
        trim_log_file(&path, 5, 10);
    }

    // ─── write_activity_entry tests ─────────────────────────────────

    #[test]
    fn write_activity_entry_creates_and_appends() {
        let pane_id = "%CLI_WRITE_TEST";
        let path = crate::activity::log_file_path(pane_id);
        let _ = fs::remove_file(&path);

        write_activity_entry(pane_id, "Read", "main.rs");
        write_activity_entry(pane_id, "Edit", "lib.rs");

        let content = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with("|Read|main.rs"));
        assert!(lines[1].ends_with("|Edit|lib.rs"));
        assert_eq!(lines[0].as_bytes()[2], b':');
        fs::remove_file(&path).ok();
    }

    #[test]
    fn write_activity_entry_sanitizes_label() {
        let pane_id = "%CLI_SANITIZE_TEST";
        let path = crate::activity::log_file_path(pane_id);
        let _ = fs::remove_file(&path);

        write_activity_entry(pane_id, "Bash", "cat file | grep foo\nbar");

        let content = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(
            lines.len(),
            1,
            "newlines in label should not create extra lines"
        );
        let label = lines[0].splitn(3, '|').nth(2).unwrap();
        assert!(!label.contains('|'));
        assert!(!label.contains('\n'));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn write_activity_entry_trims_at_threshold() {
        let pane_id = "%CLI_TRIM_TEST";
        let path = crate::activity::log_file_path(pane_id);
        let _ = fs::remove_file(&path);

        for i in 1..=215 {
            write_activity_entry(pane_id, "Read", &format!("file{}.rs", i));
        }

        let content = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert!(lines.len() <= 210, "should be trimmed, got {}", lines.len());
        assert!(lines.last().unwrap().ends_with("|Read|file215.rs"));
        fs::remove_file(&path).ok();
    }

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
}
