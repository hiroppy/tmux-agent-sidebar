use crate::tmux;

use super::label::extract_tool_label;
use super::{
    json_str, local_time_hhmm, read_stdin_json, sanitize_tmux_value, set_attention, set_status,
    tmux_pane,
};

fn set_agent_meta(pane: &str, agent: &str, json: &serde_json::Value) {
    tmux::set_pane_option(pane, "@pane_agent", agent);
    let cwd = json_str(json, "cwd");
    if !cwd.is_empty() {
        tmux::set_pane_option(pane, "@pane_cwd", cwd);
    }
    let pmode = json_str(json, "permission_mode");
    if !pmode.is_empty() {
        tmux::set_pane_option(pane, "@pane_permission_mode", pmode);
    }
}

fn clear_run_state(pane: &str) {
    tmux::unset_pane_option(pane, "@pane_started_at");
    tmux::unset_pane_option(pane, "@pane_wait_reason");
}

/// Check if a prompt is a system-injected message (not a real user prompt).
fn is_system_message(s: &str) -> bool {
    s.contains("<task-notification>")
        || s.contains("<system-reminder>")
        || s.contains("<task-")
}

fn clear_all_meta(pane: &str) {
    for key in &[
        "@pane_agent",
        "@pane_prompt",
        "@pane_prompt_source",
        "@pane_subagents",
        "@pane_cwd",
        "@pane_permission_mode",
    ] {
        tmux::unset_pane_option(pane, key);
    }
    clear_run_state(pane);
}

/// Append an agent type to a comma-separated subagent list.
fn append_subagent(current: &str, agent_type: &str) -> String {
    if current.is_empty() {
        agent_type.to_string()
    } else {
        format!("{},{}", current, agent_type)
    }
}

/// Remove the last occurrence of `agent_type` from a comma-separated list.
/// Returns `None` if not found, `Some(new_list)` otherwise (empty string if list becomes empty).
fn remove_last_subagent(current: &str, agent_type: &str) -> Option<String> {
    if current.is_empty() {
        return None;
    }
    let items: Vec<&str> = current.split(',').collect();
    let last_idx = items.iter().rposition(|&s| s == agent_type)?;
    let filtered: Vec<&str> = items
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != last_idx)
        .map(|(_, s)| *s)
        .collect();
    Some(filtered.join(","))
}

/// Parse a JSON field that may be a JSON string or an object.
fn parse_json_field(input: &serde_json::Value, field: &str) -> serde_json::Value {
    input
        .get(field)
        .and_then(|v| {
            if let Some(s) = v.as_str() {
                serde_json::from_str(s).ok()
            } else if v.is_object() {
                Some(v.clone())
            } else {
                None
            }
        })
        .unwrap_or(serde_json::Value::Null)
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
    let agent = args.first().map(|s| s.as_str()).unwrap_or("");
    let event = args.get(1).map(|s| s.as_str()).unwrap_or("");

    if agent.is_empty() || event.is_empty() {
        return 0;
    }

    match agent {
        "claude" | "codex" => {}
        _ => return 0,
    }

    let pane = tmux_pane();
    if pane.is_empty() {
        return 0;
    }

    let input = read_stdin_json();

    match event {
        "notification" => {
            set_agent_meta(&pane, agent, &input);
            let wait_reason = json_str(&input, "notification_type");
            if wait_reason == "idle_prompt" {
                return 0;
            }
            set_status(&pane, "waiting");
            set_attention(&pane, "notification");
            if !wait_reason.is_empty() {
                tmux::set_pane_option(&pane, "@pane_wait_reason", wait_reason);
            }
        }
        "stop" => {
            set_agent_meta(&pane, agent, &input);
            set_attention(&pane, "clear");
            let last_msg = json_str(&input, "last_assistant_message");
            if !last_msg.is_empty() {
                let msg = sanitize_tmux_value(last_msg);
                tmux::set_pane_option(&pane, "@pane_prompt", &msg);
                tmux::set_pane_option(&pane, "@pane_prompt_source", "response");
            }
            clear_run_state(&pane);
            set_status(&pane, "idle");

            if agent == "codex" {
                println!("{{\"continue\":true}}");
            }
        }
        "stop-failure" => {
            set_agent_meta(&pane, agent, &input);
            set_attention(&pane, "clear");
            clear_run_state(&pane);
            let error_type = json_str(&input, "error");
            let error_details = json_str(&input, "error_details");
            let reason = if !error_type.is_empty() {
                error_type
            } else {
                error_details
            };
            if !reason.is_empty() {
                tmux::set_pane_option(&pane, "@pane_wait_reason", reason);
            }
            set_status(&pane, "error");
        }
        "subagent-start" => {
            let agent_type = json_str(&input, "agent_type");
            if agent_type.is_empty() {
                return 0;
            }
            let current = tmux::get_pane_option_value(&pane, "@pane_subagents");
            let new_val = append_subagent(&current, agent_type);
            tmux::set_pane_option(&pane, "@pane_subagents", &new_val);
        }
        "subagent-stop" => {
            let agent_type = json_str(&input, "agent_type");
            if agent_type.is_empty() {
                return 0;
            }
            let current = tmux::get_pane_option_value(&pane, "@pane_subagents");
            match remove_last_subagent(&current, agent_type) {
                None => return 0,
                Some(new_val) if new_val.is_empty() => {
                    tmux::unset_pane_option(&pane, "@pane_subagents");
                }
                Some(new_val) => {
                    tmux::set_pane_option(&pane, "@pane_subagents", &new_val);
                }
            }
        }
        "user-prompt-submit" => {
            set_agent_meta(&pane, agent, &input);
            set_attention(&pane, "clear");
            set_status(&pane, "running");
            let prompt = json_str(&input, "prompt");
            if !prompt.is_empty() && !is_system_message(prompt) {
                let p = sanitize_tmux_value(prompt);
                tmux::set_pane_option(&pane, "@pane_prompt", &p);
                tmux::set_pane_option(&pane, "@pane_prompt_source", "user");
            }
            let now = unsafe { libc::time(std::ptr::null_mut()) };
            tmux::set_pane_option(&pane, "@pane_started_at", &now.to_string());
            tmux::unset_pane_option(&pane, "@pane_wait_reason");
        }
        "session-start" => {
            set_agent_meta(&pane, agent, &input);
            set_attention(&pane, "clear");
            clear_run_state(&pane);
            tmux::unset_pane_option(&pane, "@pane_prompt");
            tmux::unset_pane_option(&pane, "@pane_prompt_source");
            tmux::unset_pane_option(&pane, "@pane_subagents");
            set_status(&pane, "idle");
        }
        "session-end" => {
            set_attention(&pane, "clear");
            clear_all_meta(&pane);
            set_status(&pane, "clear");
            // Clean up activity log file
            let log_path = crate::activity::log_file_path(&pane);
            let _ = std::fs::remove_file(log_path);
        }
        "activity-log" => {
            return handle_activity_log(&pane, &input);
        }
        _ => {}
    }

    0
}

// ─── activity-log logic ─────────────────────────────────────────────────────

/// Activity-log handler, called from `hook <agent> activity-log` event.
fn handle_activity_log(pane: &str, input: &serde_json::Value) -> i32 {
    let tool_name = json_str(input, "tool_name");
    if tool_name.is_empty() {
        return 0;
    }

    let tool_input = parse_json_field(input, "tool_input");
    let tool_response = parse_json_field(input, "tool_response");
    let label = extract_tool_label(tool_name, &tool_input, &tool_response);

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
            let now = unsafe { libc::time(std::ptr::null_mut()) };
            tmux::set_pane_option(pane, "@pane_started_at", &now.to_string());
        }
    }

    // Update permission mode when plan mode tools are used
    match tool_name {
        "EnterPlanMode" => {
            tmux::set_pane_option(pane, "@pane_permission_mode", "plan");
        }
        "ExitPlanMode" => {
            tmux::set_pane_option(pane, "@pane_permission_mode", "default");
        }
        _ => {}
    }

    write_activity_entry(pane, tool_name, &label);
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    // ─── append_subagent tests ──────────────────────────────────────

    #[test]
    fn append_subagent_to_empty() {
        assert_eq!(append_subagent("", "Explore"), "Explore");
    }

    #[test]
    fn append_subagent_to_existing() {
        assert_eq!(append_subagent("Explore", "Plan"), "Explore,Plan");
    }

    #[test]
    fn append_subagent_multiple() {
        let list = append_subagent("Explore,Plan", "Explore");
        assert_eq!(list, "Explore,Plan,Explore");
    }

    // ─── remove_last_subagent tests ─────────────────────────────────

    #[test]
    fn remove_last_subagent_empty_list() {
        assert_eq!(remove_last_subagent("", "Explore"), None);
    }

    #[test]
    fn remove_last_subagent_not_found() {
        assert_eq!(remove_last_subagent("Explore,Plan", "Bash"), None);
    }

    #[test]
    fn remove_last_subagent_single_item() {
        assert_eq!(remove_last_subagent("Explore", "Explore"), Some("".into()));
    }

    #[test]
    fn remove_last_subagent_removes_last_occurrence() {
        assert_eq!(
            remove_last_subagent("Explore,Plan,Explore", "Explore"),
            Some("Explore,Plan".into())
        );
    }

    #[test]
    fn remove_last_subagent_middle_item() {
        assert_eq!(
            remove_last_subagent("Explore,Plan,Bash", "Plan"),
            Some("Explore,Bash".into())
        );
    }

    #[test]
    fn remove_last_subagent_first_item() {
        assert_eq!(
            remove_last_subagent("Plan,Explore", "Plan"),
            Some("Explore".into())
        );
    }

    #[test]
    fn remove_last_subagent_all_same_removes_last() {
        assert_eq!(
            remove_last_subagent("Explore,Explore,Explore", "Explore"),
            Some("Explore,Explore".into())
        );
    }

    // ─── parse_json_field tests ─────────────────────────────────────

    #[test]
    fn parse_json_field_object_value() {
        let input = json!({"tool_input": {"file_path": "/a/b.rs"}});
        let result = parse_json_field(&input, "tool_input");
        assert_eq!(result, json!({"file_path": "/a/b.rs"}));
    }

    #[test]
    fn parse_json_field_string_value_parses_json() {
        let input = json!({"tool_input": "{\"file_path\":\"/a/b.rs\"}"});
        let result = parse_json_field(&input, "tool_input");
        assert_eq!(result, json!({"file_path": "/a/b.rs"}));
    }

    #[test]
    fn parse_json_field_invalid_json_string_returns_null() {
        let input = json!({"tool_input": "not-json"});
        let result = parse_json_field(&input, "tool_input");
        assert!(result.is_null());
    }

    #[test]
    fn parse_json_field_missing_key_returns_null() {
        let result = parse_json_field(&json!({}), "tool_input");
        assert!(result.is_null());
    }

    #[test]
    fn parse_json_field_number_value_returns_null() {
        let input = json!({"tool_input": 42});
        let result = parse_json_field(&input, "tool_input");
        assert!(result.is_null());
    }

    #[test]
    fn parse_json_field_null_input_returns_null() {
        let result = parse_json_field(&serde_json::Value::Null, "tool_input");
        assert!(result.is_null());
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

        let input = json!({
            "tool_name": "Read",
            "tool_input": {"file_path": "/home/user/src/main.rs"}
        });
        handle_activity_log(pane_id, &input);

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("|Read|main.rs"));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn handle_activity_log_empty_tool_name_does_nothing() {
        let pane_id = "%CLI_EMPTY_TOOL";
        let path = crate::activity::log_file_path(pane_id);
        let _ = fs::remove_file(&path);

        let result = handle_activity_log(pane_id, &json!({}));
        assert_eq!(result, 0);
        assert!(!path.exists());
    }

    #[test]
    fn handle_activity_log_tool_input_as_json_string() {
        let pane_id = "%CLI_JSON_STR";
        let path = crate::activity::log_file_path(pane_id);
        let _ = fs::remove_file(&path);

        let input = json!({
            "tool_name": "Edit",
            "tool_input": "{\"file_path\":\"/a/b/test.rs\"}"
        });
        handle_activity_log(pane_id, &input);

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("|Edit|test.rs"));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn handle_activity_log_null_tool_input_uses_empty_label() {
        let pane_id = "%CLI_NULL_INPUT";
        let path = crate::activity::log_file_path(pane_id);
        let _ = fs::remove_file(&path);

        let input = json!({"tool_name": "UnknownTool"});
        handle_activity_log(pane_id, &input);

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("|UnknownTool|"));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn handle_activity_log_task_create_with_response() {
        let pane_id = "%CLI_TASK_CREATE";
        let path = crate::activity::log_file_path(pane_id);
        let _ = fs::remove_file(&path);

        let input = json!({
            "tool_name": "TaskCreate",
            "tool_input": {"subject": "Fix bug"},
            "tool_response": {"task": {"id": "42"}}
        });
        handle_activity_log(pane_id, &input);

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
    fn subagent_lifecycle_two_start_one_stop_leaves_one() {
        // Simulate: two subagents start, then one stops
        let list = append_subagent("", "Explore");
        assert_eq!(list, "Explore");

        let list = append_subagent(&list, "Explore");
        assert_eq!(list, "Explore,Explore");

        // First one completes
        let remaining = remove_last_subagent(&list, "Explore").unwrap();
        assert_eq!(remaining, "Explore");

        // Second one completes
        let remaining = remove_last_subagent(&remaining, "Explore").unwrap();
        assert_eq!(remaining, "");
    }

    #[test]
    fn subagent_lifecycle_mixed_types() {
        let list = append_subagent("", "Explore");
        let list = append_subagent(&list, "Plan");
        assert_eq!(list, "Explore,Plan");

        // Plan completes, Explore still running
        let remaining = remove_last_subagent(&list, "Plan").unwrap();
        assert_eq!(remaining, "Explore");
    }
}
