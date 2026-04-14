use serde_json::Value;

/// How a tool's label should be derived from its input/response payload.
/// Keeping this as data (rather than a giant `match` with inline closures)
/// makes adding a new tool a one-line edit in [`STRATEGY_TABLE`] and lets
/// the per-tool extraction logic live as named, individually testable
/// functions for the few cases that need custom code.
enum LabelStrategy {
    /// Unknown tool — produces an empty label.
    None,
    /// Pull a single string field straight out of `tool_input`.
    Field(&'static str),
    /// Pull a path field out of `tool_input` and reduce to its basename.
    FilePath(&'static str),
    /// Pull a URL field out of `tool_input` and strip the http(s):// prefix.
    UrlStrip(&'static str),
    /// Run a custom extractor that needs both `tool_input` and `tool_response`.
    Custom(fn(&Value, &Value) -> String),
}

/// Tool name → extraction strategy. Order is preserved for readability;
/// dispatch is O(N) but N is ~30 so a linear scan is fine and avoids the
/// overhead/lifetime constraints of a static `HashMap`.
const STRATEGY_TABLE: &[(&str, LabelStrategy)] = &[
    ("Read", LabelStrategy::FilePath("file_path")),
    ("Edit", LabelStrategy::FilePath("file_path")),
    ("Write", LabelStrategy::FilePath("file_path")),
    ("NotebookEdit", LabelStrategy::FilePath("notebook_path")),
    ("Bash", LabelStrategy::Field("command")),
    ("PowerShell", LabelStrategy::Field("command")),
    ("Glob", LabelStrategy::Field("pattern")),
    ("Grep", LabelStrategy::Field("pattern")),
    ("WebFetch", LabelStrategy::UrlStrip("url")),
    ("WebSearch", LabelStrategy::Field("query")),
    ("ToolSearch", LabelStrategy::Field("query")),
    ("Skill", LabelStrategy::Field("skill")),
    ("SendMessage", LabelStrategy::Field("to")),
    ("TeamCreate", LabelStrategy::Field("team_name")),
    ("LSP", LabelStrategy::Field("operation")),
    ("CronCreate", LabelStrategy::Field("cron")),
    ("CronDelete", LabelStrategy::Field("id")),
    ("EnterWorktree", LabelStrategy::Field("name")),
    ("ExitWorktree", LabelStrategy::Field("name")),
    ("Agent", LabelStrategy::Custom(label_agent)),
    ("TaskCreate", LabelStrategy::Custom(label_task_create)),
    ("TaskUpdate", LabelStrategy::Custom(label_task_update)),
    ("TaskGet", LabelStrategy::Custom(label_task_id)),
    ("TaskStop", LabelStrategy::Custom(label_task_id)),
    ("TaskOutput", LabelStrategy::Custom(label_task_id)),
    (
        "AskUserQuestion",
        LabelStrategy::Custom(label_ask_user_question),
    ),
];

pub(crate) fn extract_tool_label(
    tool_name: &str,
    tool_input: &Value,
    tool_response: &Value,
) -> String {
    let strategy = STRATEGY_TABLE
        .iter()
        .find(|(name, _)| *name == tool_name)
        .map(|(_, s)| s)
        .unwrap_or(&LabelStrategy::None);

    match strategy {
        LabelStrategy::None => String::new(),
        LabelStrategy::Field(key) => field_str(tool_input, key),
        LabelStrategy::FilePath(key) => basename(&field_str(tool_input, key)),
        LabelStrategy::UrlStrip(key) => {
            let url = field_str(tool_input, key);
            url.trim_start_matches("https://")
                .trim_start_matches("http://")
                .to_string()
        }
        LabelStrategy::Custom(f) => f(tool_input, tool_response),
    }
}

fn field_str(input: &Value, key: &str) -> String {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn basename(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// Subagent output: prefer the response text (`content[].type=="text"`) so
/// the Activity tab shows what came back, not just what the parent asked
/// for. Falls back to the prompt's `description` when the response is
/// missing (e.g. errors) so the entry is never blank.
fn label_agent(input: &Value, response: &Value) -> String {
    let response_text = response
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|block| block.get("type").and_then(|t| t.as_str()) == Some("text"))
        })
        .and_then(|block| block.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if response_text.is_empty() {
        field_str(input, "description")
    } else {
        response_text
    }
}

fn label_task_create(input: &Value, response: &Value) -> String {
    let task_id = response
        .get("task")
        .and_then(|t| t.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let subject = field_str(input, "subject");
    if !task_id.is_empty() {
        format!("#{task_id} {subject}")
    } else {
        subject
    }
}

fn label_task_update(input: &Value, _: &Value) -> String {
    let status = field_str(input, "status");
    let task_id = field_str(input, "taskId");
    let mut parts = Vec::new();
    if !status.is_empty() {
        parts.push(status);
    }
    if !task_id.is_empty() {
        parts.push(format!("#{task_id}"));
    }
    parts.join(" ")
}

/// Task tools (Get/Stop/Output) can use either `taskId` or `task_id`.
/// Camel-case wins when both are present, matching the legacy fall-through.
fn label_task_id(input: &Value, _: &Value) -> String {
    let id = field_str(input, "taskId");
    let id = if id.is_empty() {
        field_str(input, "task_id")
    } else {
        id
    };
    if id.is_empty() {
        String::new()
    } else {
        format!("#{id}")
    }
}

fn label_ask_user_question(input: &Value, _: &Value) -> String {
    input
        .get("questions")
        .and_then(|q| q.as_array())
        .and_then(|arr| arr.first())
        .and_then(|q| q.get("question"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn label_read_extracts_basename() {
        let input = json!({"file_path": "/home/user/project/src/main.rs"});
        assert_eq!(extract_tool_label("Read", &input, &json!(null)), "main.rs");
    }

    #[test]
    fn label_edit_extracts_basename() {
        let input = json!({"file_path": "/tmp/foo.txt"});
        assert_eq!(extract_tool_label("Edit", &input, &json!(null)), "foo.txt");
    }

    #[test]
    fn label_write_extracts_basename() {
        let input = json!({"file_path": "/a/b/c.json"});
        assert_eq!(extract_tool_label("Write", &input, &json!(null)), "c.json");
    }

    #[test]
    fn label_file_missing_path() {
        assert_eq!(extract_tool_label("Read", &json!({}), &json!(null)), "");
    }

    #[test]
    fn label_file_bare_filename() {
        let input = json!({"file_path": "README.md"});
        assert_eq!(
            extract_tool_label("Read", &input, &json!(null)),
            "README.md"
        );
    }

    #[test]
    fn label_bash_extracts_command() {
        let input = json!({"command": "cargo build"});
        assert_eq!(
            extract_tool_label("Bash", &input, &json!(null)),
            "cargo build"
        );
    }

    #[test]
    fn label_bash_preserves_long_command() {
        let cmd = "npm run test -- --watch --coverage --verbose --maxWorkers=4";
        let input = json!({"command": cmd});
        assert_eq!(extract_tool_label("Bash", &input, &json!(null)), cmd);
    }

    #[test]
    fn label_glob_extracts_pattern() {
        let input = json!({"pattern": "**/*.rs"});
        assert_eq!(extract_tool_label("Glob", &input, &json!(null)), "**/*.rs");
    }

    #[test]
    fn label_grep_extracts_pattern() {
        let input = json!({"pattern": "fn main"});
        assert_eq!(extract_tool_label("Grep", &input, &json!(null)), "fn main");
    }

    #[test]
    fn label_agent_prefers_response_text_over_description() {
        let input = json!({"description": "Search codebase"});
        let response = json!({
            "content": [
                {"type": "text", "text": "Found the bug at main.rs:42"}
            ],
            "status": "completed"
        });
        assert_eq!(
            extract_tool_label("Agent", &input, &response),
            "Found the bug at main.rs:42"
        );
    }

    #[test]
    fn label_agent_falls_back_to_description_when_no_response() {
        let input = json!({"description": "Search codebase"});
        assert_eq!(
            extract_tool_label("Agent", &input, &json!(null)),
            "Search codebase"
        );
    }

    #[test]
    fn label_agent_falls_back_to_description_when_response_has_no_text_block() {
        // tool_response exists but lacks a text content block.
        let input = json!({"description": "Deploy to staging"});
        let response = json!({"status": "completed"});
        assert_eq!(
            extract_tool_label("Agent", &input, &response),
            "Deploy to staging"
        );
    }

    #[test]
    fn label_agent_falls_back_to_description_when_text_is_empty() {
        let input = json!({"description": "Explore repo"});
        let response = json!({
            "content": [
                {"type": "text", "text": "   "}
            ]
        });
        assert_eq!(
            extract_tool_label("Agent", &input, &response),
            "Explore repo"
        );
    }

    #[test]
    fn label_agent_picks_text_block_among_mixed_types() {
        // Response can contain multiple content blocks; pick the first text one.
        let input = json!({"description": "Audit config"});
        let response = json!({
            "content": [
                {"type": "tool_use", "id": "x"},
                {"type": "text", "text": "No drift detected"}
            ]
        });
        assert_eq!(
            extract_tool_label("Agent", &input, &response),
            "No drift detected"
        );
    }

    #[test]
    fn label_agent_preserves_multiline_response() {
        // Multi-line responses are kept as-is; sanitization happens later in
        // hook.rs::write_activity_entry.
        let input = json!({"description": "List files"});
        let response = json!({
            "content": [
                {"type": "text", "text": "main.rs\nlib.rs\ntmux.rs"}
            ]
        });
        assert_eq!(
            extract_tool_label("Agent", &input, &response),
            "main.rs\nlib.rs\ntmux.rs"
        );
    }

    #[test]
    fn label_webfetch_strips_https() {
        let input = json!({"url": "https://example.com/docs"});
        assert_eq!(
            extract_tool_label("WebFetch", &input, &json!(null)),
            "example.com/docs"
        );
    }

    #[test]
    fn label_webfetch_strips_http() {
        let input = json!({"url": "http://example.com"});
        assert_eq!(
            extract_tool_label("WebFetch", &input, &json!(null)),
            "example.com"
        );
    }

    #[test]
    fn label_webfetch_no_protocol_unchanged() {
        let input = json!({"url": "example.com/path"});
        assert_eq!(
            extract_tool_label("WebFetch", &input, &json!(null)),
            "example.com/path"
        );
    }

    #[test]
    fn label_websearch_extracts_query() {
        let input = json!({"query": "rust tutorial"});
        assert_eq!(
            extract_tool_label("WebSearch", &input, &json!(null)),
            "rust tutorial"
        );
    }

    #[test]
    fn label_skill_extracts_skill() {
        let input = json!({"skill": "commit"});
        assert_eq!(extract_tool_label("Skill", &input, &json!(null)), "commit");
    }

    #[test]
    fn label_toolsearch_extracts_query() {
        let input = json!({"query": "select:Read"});
        assert_eq!(
            extract_tool_label("ToolSearch", &input, &json!(null)),
            "select:Read"
        );
    }

    #[test]
    fn label_task_create_with_id() {
        let input = json!({"subject": "Add feature"});
        let response = json!({"task": {"id": "1"}});
        assert_eq!(
            extract_tool_label("TaskCreate", &input, &response),
            "#1 Add feature"
        );
    }

    #[test]
    fn label_task_create_without_id() {
        let input = json!({"subject": "Add feature"});
        assert_eq!(
            extract_tool_label("TaskCreate", &input, &json!(null)),
            "Add feature"
        );
    }

    #[test]
    fn label_task_create_empty_subject_with_id() {
        let input = json!({});
        let response = json!({"task": {"id": "5"}});
        assert_eq!(extract_tool_label("TaskCreate", &input, &response), "#5 ");
    }

    #[test]
    fn label_task_update_status_and_id() {
        let input = json!({"status": "completed", "taskId": "3"});
        assert_eq!(
            extract_tool_label("TaskUpdate", &input, &json!(null)),
            "completed #3"
        );
    }

    #[test]
    fn label_task_update_status_only() {
        let input = json!({"status": "in_progress"});
        assert_eq!(
            extract_tool_label("TaskUpdate", &input, &json!(null)),
            "in_progress"
        );
    }

    #[test]
    fn label_task_update_id_only() {
        let input = json!({"taskId": "7"});
        assert_eq!(extract_tool_label("TaskUpdate", &input, &json!(null)), "#7");
    }

    #[test]
    fn label_task_update_empty() {
        assert_eq!(
            extract_tool_label("TaskUpdate", &json!({}), &json!(null)),
            ""
        );
    }

    #[test]
    fn label_task_get_with_task_id() {
        let input = json!({"taskId": "5"});
        assert_eq!(extract_tool_label("TaskGet", &input, &json!(null)), "#5");
    }

    #[test]
    fn label_task_stop_with_task_id() {
        let input = json!({"task_id": "7"});
        assert_eq!(extract_tool_label("TaskStop", &input, &json!(null)), "#7");
    }

    #[test]
    fn label_task_get_prefers_task_id_camel_case() {
        let input = json!({"taskId": "1", "task_id": "2"});
        assert_eq!(extract_tool_label("TaskGet", &input, &json!(null)), "#1");
    }

    #[test]
    fn label_task_output_empty() {
        assert_eq!(
            extract_tool_label("TaskOutput", &json!({}), &json!(null)),
            ""
        );
    }

    #[test]
    fn label_send_message() {
        let input = json!({"to": "agent-1"});
        assert_eq!(
            extract_tool_label("SendMessage", &input, &json!(null)),
            "agent-1"
        );
    }

    #[test]
    fn label_team_create() {
        let input = json!({"team_name": "reviewers"});
        assert_eq!(
            extract_tool_label("TeamCreate", &input, &json!(null)),
            "reviewers"
        );
    }

    #[test]
    fn label_notebook_edit() {
        let input = json!({"notebook_path": "/home/user/analysis.ipynb"});
        assert_eq!(
            extract_tool_label("NotebookEdit", &input, &json!(null)),
            "analysis.ipynb"
        );
    }

    #[test]
    fn label_lsp() {
        let input = json!({"operation": "hover"});
        assert_eq!(extract_tool_label("LSP", &input, &json!(null)), "hover");
    }

    #[test]
    fn label_ask_user_question() {
        let input = json!({"questions": [{"question": "Which option?"}]});
        assert_eq!(
            extract_tool_label("AskUserQuestion", &input, &json!(null)),
            "Which option?"
        );
    }

    #[test]
    fn label_ask_user_question_empty_array() {
        assert_eq!(
            extract_tool_label("AskUserQuestion", &json!({"questions": []}), &json!(null)),
            ""
        );
    }

    #[test]
    fn label_ask_user_question_no_questions_key() {
        assert_eq!(
            extract_tool_label("AskUserQuestion", &json!({}), &json!(null)),
            ""
        );
    }

    #[test]
    fn label_cron_create() {
        let input = json!({"cron": "*/5 * * * *"});
        assert_eq!(
            extract_tool_label("CronCreate", &input, &json!(null)),
            "*/5 * * * *"
        );
    }

    #[test]
    fn label_cron_delete() {
        let input = json!({"id": "abc123"});
        assert_eq!(
            extract_tool_label("CronDelete", &input, &json!(null)),
            "abc123"
        );
    }

    #[test]
    fn label_enter_worktree() {
        let input = json!({"name": "feature-branch"});
        assert_eq!(
            extract_tool_label("EnterWorktree", &input, &json!(null)),
            "feature-branch"
        );
    }

    #[test]
    fn label_powershell_extracts_command() {
        let input = json!({"command": "Get-Process"});
        assert_eq!(
            extract_tool_label("PowerShell", &input, &json!(null)),
            "Get-Process"
        );
    }

    #[test]
    fn label_exit_worktree() {
        let input = json!({"name": "feature-branch"});
        assert_eq!(
            extract_tool_label("ExitWorktree", &input, &json!(null)),
            "feature-branch"
        );
    }

    #[test]
    fn label_powershell_empty_command() {
        assert_eq!(
            extract_tool_label("PowerShell", &json!({}), &json!(null)),
            ""
        );
    }

    #[test]
    fn label_exit_worktree_empty_name() {
        assert_eq!(
            extract_tool_label("ExitWorktree", &json!({}), &json!(null)),
            ""
        );
    }

    #[test]
    fn label_unknown_tool_returns_empty() {
        assert_eq!(
            extract_tool_label("UnknownTool", &json!({"anything": "value"}), &json!(null)),
            ""
        );
    }

    #[test]
    fn label_null_inputs() {
        assert_eq!(extract_tool_label("Read", &json!(null), &json!(null)), "");
        assert_eq!(
            extract_tool_label("TaskCreate", &json!(null), &json!(null)),
            ""
        );
        assert_eq!(extract_tool_label("Bash", &json!(null), &json!(null)), "");
        assert_eq!(
            extract_tool_label("WebFetch", &json!(null), &json!(null)),
            ""
        );
    }

    #[test]
    fn label_cron_list_is_unknown() {
        assert_eq!(extract_tool_label("CronList", &json!({}), &json!(null)), "");
    }

    #[test]
    fn label_exit_plan_mode_is_unknown() {
        assert_eq!(
            extract_tool_label("ExitPlanMode", &json!({}), &json!(null)),
            ""
        );
    }

    #[test]
    fn label_team_delete_is_unknown() {
        assert_eq!(
            extract_tool_label("TeamDelete", &json!({}), &json!(null)),
            ""
        );
    }
}
