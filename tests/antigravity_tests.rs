use serde_json::json;
use tmux_agent_sidebar::event::{AgentEvent, ExecutionState, resolve_adapter};

#[test]
fn antigravity_normalizes_native_tool_calls() {
    let adapter = resolve_adapter("agy").expect("Antigravity adapter");
    for (name, args, expected_name, expected_input) in [
        (
            "run_command",
            json!({"CommandLine":"cargo test"}),
            "Bash",
            json!({"command":"cargo test"}),
        ),
        (
            "view_file",
            json!({"AbsolutePath":"/repo/src/main.rs"}),
            "Read",
            json!({"file_path":"/repo/src/main.rs"}),
        ),
        (
            "write_to_file",
            json!({"TargetFile":"/repo/new.rs"}),
            "Write",
            json!({"file_path":"/repo/new.rs"}),
        ),
        (
            "replace_file_content",
            json!({"TargetFile":"/repo/main.rs"}),
            "Edit",
            json!({"file_path":"/repo/main.rs"}),
        ),
        (
            "multi_replace_file_content",
            json!({"TargetFile":"/repo/main.rs"}),
            "Edit",
            json!({"file_path":"/repo/main.rs"}),
        ),
        (
            "grep_search",
            json!({"Query":"TODO"}),
            "Grep",
            json!({"pattern":"TODO"}),
        ),
        (
            "find_by_name",
            json!({"Pattern":"*.rs"}),
            "Glob",
            json!({"pattern":"*.rs"}),
        ),
    ] {
        assert_eq!(
            adapter.parse(
                "activity-log",
                &json!({
                    "conversationId":"conversation-1", "workspacePaths":["/repo"],
                    "toolCall":{"name":name,"args":args}
                })
            ),
            Some(AgentEvent::ActivityLog {
                tool_name: expected_name.into(),
                tool_input: expected_input,
                tool_response: serde_json::Value::Null
            })
        );
    }
}

#[test]
fn antigravity_rejects_missing_identity_and_unsupported_events() {
    let adapter = resolve_adapter("agy").expect("Antigravity adapter");
    for event in [
        "execution-update",
        "activity-log",
        "session-start",
        "notification",
    ] {
        assert!(adapter.parse(event, &json!({})).is_none());
    }
    assert!(
        adapter
            .parse("session-start", &json!({"conversationId":"c"}))
            .is_none()
    );
}

#[test]
fn antigravity_execution_states_follow_native_payloads() {
    let adapter = resolve_adapter("agy").unwrap();
    for (fields, state) in [
        (json!({"invocationNum":0}), ExecutionState::Running),
        (json!({"invocationNum":5}), ExecutionState::Running),
        (
            json!({"terminationReason":"model_stop","fullyIdle":false}),
            ExecutionState::Background,
        ),
        (
            json!({"terminationReason":"model_stop","fullyIdle":true}),
            ExecutionState::Idle,
        ),
        (
            json!({"terminationReason":"error","error":"quota exceeded","fullyIdle":true}),
            ExecutionState::Failed("quota exceeded".into()),
        ),
        (
            json!({"terminationReason":"max_steps_exceeded","fullyIdle":true}),
            ExecutionState::Failed("max_steps_exceeded".into()),
        ),
    ] {
        let mut payload = json!({"conversationId":"c1","workspacePaths":["/repo","/second"]});
        payload
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        assert_eq!(
            adapter.parse("execution-update", &payload),
            Some(AgentEvent::ExecutionUpdate {
                agent: "agy".into(),
                cwd: "/repo".into(),
                session_id: "c1".into(),
                state
            })
        );
    }
    for payload in [
        json!({"conversationId":"c1"}),
        json!({"conversationId":"c1","terminationReason":"model_stop"}),
        json!({"conversationId":"c1","terminationReason":"model_stop","fullyIdle":"true"}),
        json!({"conversationId":"c1","invocationNum":-1}),
    ] {
        assert!(adapter.parse("execution-update", &payload).is_none());
    }
}

#[test]
fn antigravity_preserves_unknown_tools_without_fabricating_output() {
    let event = resolve_adapter("agy").unwrap().parse(
        "activity-log",
        &json!({
            "conversationId":"c1", "toolCall":{"name":"mcp.example","args":{"query":"hi"}},
            "error":"failed"
        }),
    );
    assert_eq!(
        event,
        Some(AgentEvent::ActivityLog {
            tool_name: "mcp.example".into(),
            tool_input: json!({"query":"hi"}),
            tool_response: serde_json::Value::Null
        })
    );
}

#[test]
fn antigravity_plugin_contract_and_safe_responses() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    use tmux_agent_sidebar::adapter::antigravity::AntigravityAdapter;

    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../plugins/antigravity/plugin.json")).unwrap();
    assert_eq!(manifest["name"], "tmux-agent-sidebar");
    let hooks: serde_json::Value =
        serde_json::from_str(include_str!("../plugins/antigravity/hooks.json")).unwrap();
    let hooks = hooks["tmux-agent-sidebar"].as_object().unwrap();
    assert_eq!(hooks.len(), AntigravityAdapter::HOOK_REGISTRATIONS.len());
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("hook ' $ with spaces.sh");
    // Simulate a noisy/failing sidebar without touching any real tmux session.
    std::fs::write(&script, "read -r payload\nprintf '%s\\n' \"$1\" \"$2\" \"$TMUX_PANE\" \"$payload\" > \"$CAPTURE\"\nprintf 'unwanted stdout'\nexit 23\n").unwrap();
    for reg in AntigravityAdapter::HOOK_REGISTRATIONS {
        let action = if reg.matcher.is_some() {
            &hooks[reg.trigger][0]["hooks"][0]
        } else {
            &hooks[reg.trigger][0]
        };
        assert_eq!(action["type"], "command");
        let capture = dir.path().join("capture");
        for missing in [false, true] {
            let mut child = Command::new("bash")
                .args(["-c", action["command"].as_str().unwrap()])
                .env(
                    "TMUX_AGENT_SIDEBAR_HOOK",
                    if missing {
                        dir.path().join("missing.sh")
                    } else {
                        script.clone()
                    },
                )
                .env("TMUX_PANE", "%FAKE_AGY")
                .env("CAPTURE", &capture)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            if !missing {
                child
                    .stdin
                    .take()
                    .unwrap()
                    .write_all(b"{\"conversationId\":\"c1\"}\n")
                    .unwrap();
            }
            let output = child.wait_with_output().unwrap();
            assert!(output.status.success());
            assert!(output.stderr.is_empty());
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
                if reg.trigger == "Stop" {
                    json!({"decision":"stop"})
                } else {
                    json!({})
                }
            );
            if !missing {
                assert_eq!(
                    std::fs::read_to_string(&capture).unwrap(),
                    format!(
                        "agy\n{}\n%FAKE_AGY\n{{\"conversationId\":\"c1\"}}\n",
                        reg.kind.external_name()
                    )
                );
            }
        }
    }
}

#[test]
fn antigravity_agent_identity_color_and_spawn() {
    use tmux_agent_sidebar::{tmux::AgentType, ui::colors::ColorTheme, worktree};
    assert_eq!(AgentType::from_label("agy"), Some(AgentType::Antigravity));
    assert_eq!(AgentType::Antigravity.label(), "agy");
    assert_eq!(
        ColorTheme::default().agent_color(&AgentType::Antigravity),
        ratatui::style::Color::Indexed(150)
    );
    assert!(worktree::AGENTS.contains(&"agy"));
    assert_eq!(worktree::modes_for("agy"), &["default"]);
    assert_eq!(worktree::agent_command("agy", "default"), "agy");
    assert_eq!(worktree::agent_command("agy", "bypassPermissions"), "agy");
}
