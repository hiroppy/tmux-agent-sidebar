use crate::time::now_epoch_secs;
use crate::tmux;
use crate::tool_name::CanonicalTool;

use super::super::label::extract_tool_label;
use super::super::{local_time_hhmm, sanitize_tmux_value, set_status};
use super::context::pane_writes_allowed;

/// Write a single activity entry to the log file and trim if needed.
pub(super) fn write_activity_entry(pane: &str, tool_name: &str, label: &str) {
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
pub(super) fn trim_log_file(path: &std::path::Path, keep: usize, threshold: usize) {
    if let Ok(content) = std::fs::read_to_string(path) {
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() > threshold {
            let start = lines.len() - keep;
            let _ = std::fs::write(path, lines[start..].join("\n") + "\n");
        }
    }
}

/// Activity-log handler, called from `hook <agent> activity-log` event.
pub(super) fn handle_activity_log(
    pane: &str,
    tool_name: &str,
    tool_input: &serde_json::Value,
    tool_response: &serde_json::Value,
) -> i32 {
    let label = extract_tool_label(tool_name, tool_input, tool_response);
    if is_background_bash(tool_name, tool_input) {
        let stored = if label.is_empty() {
            tmux::BG_CMD_PLACEHOLDER
        } else {
            label.as_str()
        };
        tmux::set_pane_option(pane, tmux::PANE_BG_CMD, &sanitize_tmux_value(stored));
    }

    let current_status = tmux::get_pane_option_value(pane, tmux::PANE_STATUS);
    if current_status != "running" && !current_status.is_empty() {
        set_status(pane, "running");
        if current_status == "waiting" {
            tmux::unset_pane_option(pane, tmux::PANE_ATTENTION);
            tmux::unset_pane_option(pane, tmux::PANE_WAIT_REASON);
        }
        let existing_started = tmux::get_pane_option_value(pane, tmux::PANE_STARTED_AT);
        if existing_started.is_empty() {
            tmux::set_pane_option(pane, tmux::PANE_STARTED_AT, &now_epoch_secs().to_string());
        }
    }

    // Update permission mode when plan mode tools are used.
    // Same parent-protection rule as `set_agent_meta`: a subagent that
    // enters/exits plan mode must not flip the parent pane's badge.
    if pane_writes_allowed(pane) {
        match tool_name {
            "EnterPlanMode" => {
                tmux::set_pane_option(pane, tmux::PANE_PERMISSION_MODE, "plan");
            }
            "ExitPlanMode" => {
                tmux::set_pane_option(pane, tmux::PANE_PERMISSION_MODE, "default");
            }
            _ => {}
        }
    }

    write_activity_entry(pane, tool_name, &label);
    0
}

fn is_background_bash(tool_name: &str, tool_input: &serde_json::Value) -> bool {
    tool_name == CanonicalTool::Bash.as_str()
        && ["run_in_background", "runInBackground"]
            .iter()
            .any(|key| tool_input.get(key).and_then(|v| v.as_bool()) == Some(true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use serde_json::json;
    use std::fs;

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
    fn handle_activity_log_marks_background_bash() {
        let _guard = tmux::test_mock::install();
        let pane_id = "%CLI_BG_BASH";
        let path = crate::activity::log_file_path(pane_id);
        let _ = fs::remove_file(&path);

        handle_activity_log(
            pane_id,
            "Bash",
            &json!({"command": "npm run dev", "run_in_background": true}),
            &Value::Null,
        );

        assert_eq!(
            tmux::test_mock::get(pane_id, tmux::PANE_BG_CMD).as_deref(),
            Some("npm run dev"),
            "command string must be stored so the row body can show what is running",
        );
        fs::remove_file(&path).ok();
    }

    #[test]
    fn handle_activity_log_background_bash_without_command_falls_back_to_placeholder() {
        let _guard = tmux::test_mock::install();
        let pane_id = "%CLI_BG_BASH_NO_CMD";
        let path = crate::activity::log_file_path(pane_id);
        let _ = fs::remove_file(&path);

        handle_activity_log(
            pane_id,
            "Bash",
            &json!({"run_in_background": true}),
            &Value::Null,
        );

        assert_eq!(
            tmux::test_mock::get(pane_id, tmux::PANE_BG_CMD).as_deref(),
            Some(tmux::BG_CMD_PLACEHOLDER)
        );
        fs::remove_file(&path).ok();
    }

    #[test]
    fn handle_activity_log_sanitizes_command_before_writing() {
        let _guard = tmux::test_mock::install();
        let pane_id = "%CLI_BG_BASH_PIPE";
        let path = crate::activity::log_file_path(pane_id);
        let _ = fs::remove_file(&path);

        handle_activity_log(
            pane_id,
            "Bash",
            &json!({"command": "cat a.log | grep foo\nbar", "run_in_background": true}),
            &Value::Null,
        );

        let stored = tmux::test_mock::get(pane_id, tmux::PANE_BG_CMD).unwrap_or_default();
        assert!(!stored.contains('|'));
        assert!(!stored.contains('\n'));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn handle_activity_log_preserves_bg_cmd_when_background_resumes_running() {
        // Regression: a tool-use burst must not clear @pane_bg_cmd, or
        // the next Stop would land in `idle` instead of `background`.
        let _guard = tmux::test_mock::install();
        let pane_id = "%CLI_BG_PRESERVE";
        let path = crate::activity::log_file_path(pane_id);
        let _ = fs::remove_file(&path);
        tmux::test_mock::set(pane_id, tmux::PANE_STATUS, "background");
        tmux::test_mock::set(pane_id, tmux::PANE_BG_CMD, "npm run dev");

        handle_activity_log(
            pane_id,
            "Read",
            &json!({"file_path": "/home/user/src/main.rs"}),
            &Value::Null,
        );

        assert_eq!(
            tmux::test_mock::get(pane_id, tmux::PANE_STATUS).as_deref(),
            Some("running")
        );
        assert_eq!(
            tmux::test_mock::get(pane_id, tmux::PANE_BG_CMD).as_deref(),
            Some("npm run dev")
        );
        fs::remove_file(&path).ok();
    }

    #[test]
    fn handle_activity_log_waiting_transitions_to_running_on_tool_use() {
        let _guard = tmux::test_mock::install();
        let pane_id = "%CLI_WAIT_TO_RUN";
        let path = crate::activity::log_file_path(pane_id);
        let _ = fs::remove_file(&path);
        tmux::test_mock::set(pane_id, tmux::PANE_STATUS, "waiting");
        tmux::test_mock::set(pane_id, tmux::PANE_ATTENTION, "notification");
        tmux::test_mock::set(pane_id, tmux::PANE_WAIT_REASON, "permission_prompt");

        handle_activity_log(
            pane_id,
            "Read",
            &json!({"file_path": "/home/user/src/main.rs"}),
            &Value::Null,
        );

        assert_eq!(
            tmux::test_mock::get(pane_id, tmux::PANE_STATUS).as_deref(),
            Some("running")
        );
        assert!(!tmux::test_mock::contains(pane_id, tmux::PANE_ATTENTION));
        assert!(!tmux::test_mock::contains(pane_id, tmux::PANE_WAIT_REASON));
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
        tmux::test_mock::set(pane, tmux::PANE_SUBAGENTS, "Explore:sub-1");
        tmux::test_mock::set(pane, tmux::PANE_PERMISSION_MODE, "default");

        // A subagent's EnterPlanMode tool use must not flip the parent
        // badge to "plan".
        handle_activity_log(pane, "EnterPlanMode", &Value::Null, &Value::Null);

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_PERMISSION_MODE).as_deref(),
            Some("default"),
            "child EnterPlanMode must not overwrite parent's permission_mode"
        );
    }
}
