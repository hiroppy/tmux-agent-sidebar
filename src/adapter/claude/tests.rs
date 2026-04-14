use super::*;
use serde_json::json;

#[test]
fn hook_registrations_match_parse_arms() {
    super::super::assert_table_drift_free("claude", ClaudeAdapter::HOOK_REGISTRATIONS);
}

#[test]
fn session_start() {
    let adapter = ClaudeAdapter;
    let input = json!({"cwd": "/home/user", "permission_mode": "default"});
    let event = adapter.parse("session-start", &input).unwrap();
    assert_eq!(
        event,
        AgentEvent::SessionStart {
            agent: "claude".into(),
            cwd: "/home/user".into(),
            permission_mode: "default".into(),
            worktree: None,
            agent_id: None,
            session_id: None,
        }
    );
}

#[test]
fn session_end() {
    let adapter = ClaudeAdapter;
    assert_eq!(
        adapter.parse("session-end", &json!({})).unwrap(),
        AgentEvent::SessionEnd
    );
}

#[test]
fn user_prompt_submit() {
    let adapter = ClaudeAdapter;
    let input = json!({"cwd": "/tmp", "permission_mode": "auto", "prompt": "fix bug"});
    let event = adapter.parse("user-prompt-submit", &input).unwrap();
    assert_eq!(
        event,
        AgentEvent::UserPromptSubmit {
            agent: "claude".into(),
            cwd: "/tmp".into(),
            permission_mode: "auto".into(),
            prompt: "fix bug".into(),
            worktree: None,
            agent_id: None,
            session_id: None,
        }
    );
}

#[test]
fn notification() {
    let adapter = ClaudeAdapter;
    let input =
        json!({"cwd": "/tmp", "permission_mode": "default", "notification_type": "permission"});
    let event = adapter.parse("notification", &input).unwrap();
    assert_eq!(
        event,
        AgentEvent::Notification {
            agent: "claude".into(),
            cwd: "/tmp".into(),
            permission_mode: "default".into(),
            wait_reason: "permission".into(),
            meta_only: false,
            worktree: None,
            agent_id: None,
            session_id: None,
        }
    );
}

#[test]
fn notification_idle_prompt_is_meta_only() {
    let adapter = ClaudeAdapter;
    let input =
        json!({"cwd": "/tmp", "permission_mode": "default", "notification_type": "idle_prompt"});
    let event = adapter.parse("notification", &input).unwrap();
    assert_eq!(
        event,
        AgentEvent::Notification {
            agent: "claude".into(),
            cwd: "/tmp".into(),
            permission_mode: "default".into(),
            wait_reason: "idle_prompt".into(),
            meta_only: true,
            worktree: None,
            agent_id: None,
            session_id: None,
        }
    );
}

#[test]
fn stop() {
    let adapter = ClaudeAdapter;
    let input =
        json!({"cwd": "/tmp", "permission_mode": "default", "last_assistant_message": "done"});
    let event = adapter.parse("stop", &input).unwrap();
    assert_eq!(
        event,
        AgentEvent::Stop {
            agent: "claude".into(),
            cwd: "/tmp".into(),
            permission_mode: "default".into(),
            last_message: "done".into(),
            response: None,
            worktree: None,
            agent_id: None,
            session_id: None,
        }
    );
}

#[test]
fn stop_failure_upstream_error_type_field() {
    let adapter = ClaudeAdapter;
    let input = json!({"cwd": "/tmp", "permission_mode": "default", "error_type": "rate_limit", "error_message": "too many requests"});
    let event = adapter.parse("stop-failure", &input).unwrap();
    assert_eq!(
        event,
        AgentEvent::StopFailure {
            agent: "claude".into(),
            cwd: "/tmp".into(),
            permission_mode: "default".into(),
            error: "rate_limit".into(),
            worktree: None,
            agent_id: None,
            session_id: None,
        }
    );
}

#[test]
fn stop_failure_legacy_error_field() {
    let adapter = ClaudeAdapter;
    let input = json!({"cwd": "/tmp", "permission_mode": "default", "error": "rate_limit", "error_details": "too many"});
    let event = adapter.parse("stop-failure", &input).unwrap();
    assert_eq!(
        event,
        AgentEvent::StopFailure {
            agent: "claude".into(),
            cwd: "/tmp".into(),
            permission_mode: "default".into(),
            error: "rate_limit".into(),
            worktree: None,
            agent_id: None,
            session_id: None,
        }
    );
}

#[test]
fn stop_failure_falls_back_to_error_message() {
    let adapter = ClaudeAdapter;
    let input = json!({"cwd": "/tmp", "permission_mode": "default", "error_message": "something went wrong"});
    let event = adapter.parse("stop-failure", &input).unwrap();
    assert_eq!(
        event,
        AgentEvent::StopFailure {
            agent: "claude".into(),
            cwd: "/tmp".into(),
            permission_mode: "default".into(),
            error: "something went wrong".into(),
            worktree: None,
            agent_id: None,
            session_id: None,
        }
    );
}

#[test]
fn stop_failure_falls_back_to_error_details() {
    let adapter = ClaudeAdapter;
    let input = json!({"cwd": "/tmp", "permission_mode": "default", "error_details": "something went wrong"});
    let event = adapter.parse("stop-failure", &input).unwrap();
    assert_eq!(
        event,
        AgentEvent::StopFailure {
            agent: "claude".into(),
            cwd: "/tmp".into(),
            permission_mode: "default".into(),
            error: "something went wrong".into(),
            worktree: None,
            agent_id: None,
            session_id: None,
        }
    );
}

#[test]
fn subagent_start() {
    let adapter = ClaudeAdapter;
    let input = json!({"agent_type": "Explore"});
    assert_eq!(
        adapter.parse("subagent-start", &input).unwrap(),
        AgentEvent::SubagentStart {
            agent_type: "Explore".into(),
            agent_id: None,
        }
    );
}

#[test]
fn subagent_start_captures_agent_id() {
    let adapter = ClaudeAdapter;
    let input = json!({"agent_type": "Explore", "agent_id": "sub-42"});
    assert_eq!(
        adapter.parse("subagent-start", &input).unwrap(),
        AgentEvent::SubagentStart {
            agent_type: "Explore".into(),
            agent_id: Some("sub-42".into()),
        }
    );
}

#[test]
fn subagent_start_empty_type_ignored() {
    let adapter = ClaudeAdapter;
    assert!(adapter.parse("subagent-start", &json!({})).is_none());
}

#[test]
fn subagent_stop() {
    let adapter = ClaudeAdapter;
    let input = json!({"agent_type": "Plan"});
    assert_eq!(
        adapter.parse("subagent-stop", &input).unwrap(),
        AgentEvent::SubagentStop {
            agent_type: "Plan".into(),
            agent_id: None,
            last_message: "".into(),
            transcript_path: "".into(),
        }
    );
}

#[test]
fn subagent_stop_captures_full_payload() {
    let adapter = ClaudeAdapter;
    let input = json!({
        "agent_type": "Explore",
        "agent_id": "sub-42",
        "last_assistant_message": "Found the bug at main.rs:42",
        "agent_transcript_path": "/tmp/sub-transcript.json"
    });
    assert_eq!(
        adapter.parse("subagent-stop", &input).unwrap(),
        AgentEvent::SubagentStop {
            agent_type: "Explore".into(),
            agent_id: Some("sub-42".into()),
            last_message: "Found the bug at main.rs:42".into(),
            transcript_path: "/tmp/sub-transcript.json".into(),
        }
    );
}

#[test]
fn activity_log() {
    let adapter = ClaudeAdapter;
    let input = json!({"tool_name": "Read", "tool_input": {"file_path": "/a/b.rs"}});
    let event = adapter.parse("activity-log", &input).unwrap();
    assert_eq!(
        event,
        AgentEvent::ActivityLog {
            tool_name: "Read".into(),
            tool_input: json!({"file_path": "/a/b.rs"}),
            tool_response: Value::Null,
        }
    );
}

#[test]
fn activity_log_string_tool_input() {
    let adapter = ClaudeAdapter;
    let input = json!({"tool_name": "Edit", "tool_input": "{\"file_path\":\"/a/b.rs\"}"});
    let event = adapter.parse("activity-log", &input).unwrap();
    assert_eq!(
        event,
        AgentEvent::ActivityLog {
            tool_name: "Edit".into(),
            tool_input: json!({"file_path": "/a/b.rs"}),
            tool_response: Value::Null,
        }
    );
}

#[test]
fn activity_log_empty_tool_name_ignored() {
    let adapter = ClaudeAdapter;
    assert!(adapter.parse("activity-log", &json!({})).is_none());
}

#[test]
fn task_created() {
    let adapter = ClaudeAdapter;
    let input = json!({"task_id": "42", "task_subject": "Fix bug"});
    assert_eq!(
        adapter.parse("task-created", &input).unwrap(),
        AgentEvent::TaskCreated {
            task_id: "42".into(),
            task_subject: "Fix bug".into(),
        }
    );
}

#[test]
fn task_completed() {
    let adapter = ClaudeAdapter;
    let input = json!({"task_id": "42", "task_subject": "Fix bug"});
    assert_eq!(
        adapter.parse("task-completed", &input).unwrap(),
        AgentEvent::TaskCompleted {
            task_id: "42".into(),
            task_subject: "Fix bug".into(),
        }
    );
}

#[test]
fn teammate_idle() {
    let adapter = ClaudeAdapter;
    let input = json!({"teammate_name": "reviewer", "team_name": "dev"});
    assert_eq!(
        adapter.parse("teammate-idle", &input).unwrap(),
        AgentEvent::TeammateIdle {
            teammate_name: "reviewer".into(),
            team_name: "dev".into(),
        }
    );
}

#[test]
fn worktree_create() {
    let adapter = ClaudeAdapter;
    assert_eq!(
        adapter.parse("worktree-create", &json!({})).unwrap(),
        AgentEvent::WorktreeCreate
    );
}

#[test]
fn worktree_remove() {
    let adapter = ClaudeAdapter;
    let input = json!({"worktree_path": "/tmp/wt"});
    assert_eq!(
        adapter.parse("worktree-remove", &input).unwrap(),
        AgentEvent::WorktreeRemove {
            worktree_path: "/tmp/wt".into(),
        }
    );
}

#[test]
fn task_created_empty_fields() {
    let adapter = ClaudeAdapter;
    assert_eq!(
        adapter.parse("task-created", &json!({})).unwrap(),
        AgentEvent::TaskCreated {
            task_id: "".into(),
            task_subject: "".into(),
        }
    );
}

#[test]
fn task_completed_empty_fields() {
    let adapter = ClaudeAdapter;
    assert_eq!(
        adapter.parse("task-completed", &json!({})).unwrap(),
        AgentEvent::TaskCompleted {
            task_id: "".into(),
            task_subject: "".into(),
        }
    );
}

#[test]
fn teammate_idle_empty_fields() {
    let adapter = ClaudeAdapter;
    assert_eq!(
        adapter.parse("teammate-idle", &json!({})).unwrap(),
        AgentEvent::TeammateIdle {
            teammate_name: "".into(),
            team_name: "".into(),
        }
    );
}

#[test]
fn worktree_remove_empty_path() {
    let adapter = ClaudeAdapter;
    assert_eq!(
        adapter.parse("worktree-remove", &json!({})).unwrap(),
        AgentEvent::WorktreeRemove {
            worktree_path: "".into(),
        }
    );
}

#[test]
fn task_created_full_upstream_payload() {
    let adapter = ClaudeAdapter;
    let input = json!({
        "session_id": "sess-1",
        "transcript_path": "/tmp/transcript",
        "cwd": "/home/user/project",
        "permission_mode": "auto",
        "hook_event_name": "TaskCreated",
        "task_id": "99",
        "task_subject": "Deploy to staging",
        "task_description": "Run deployment pipeline",
        "teammate_name": "deployer",
        "team_name": "ops"
    });
    let event = adapter.parse("task-created", &input).unwrap();
    assert_eq!(
        event,
        AgentEvent::TaskCreated {
            task_id: "99".into(),
            task_subject: "Deploy to staging".into(),
        }
    );
}

#[test]
fn task_completed_full_upstream_payload() {
    let adapter = ClaudeAdapter;
    let input = json!({
        "session_id": "sess-1",
        "transcript_path": "/tmp/transcript",
        "cwd": "/home/user/project",
        "permission_mode": "auto",
        "hook_event_name": "TaskCompleted",
        "task_id": "99",
        "task_subject": "Deploy to staging",
        "teammate_name": "deployer",
        "team_name": "ops"
    });
    let event = adapter.parse("task-completed", &input).unwrap();
    assert_eq!(
        event,
        AgentEvent::TaskCompleted {
            task_id: "99".into(),
            task_subject: "Deploy to staging".into(),
        }
    );
}

#[test]
fn teammate_idle_full_upstream_payload() {
    let adapter = ClaudeAdapter;
    let input = json!({
        "session_id": "sess-1",
        "transcript_path": "/tmp/transcript",
        "cwd": "/home/user/project",
        "permission_mode": "auto",
        "hook_event_name": "TeammateIdle",
        "teammate_name": "code-reviewer",
        "team_name": "review-team"
    });
    let event = adapter.parse("teammate-idle", &input).unwrap();
    assert_eq!(
        event,
        AgentEvent::TeammateIdle {
            teammate_name: "code-reviewer".into(),
            team_name: "review-team".into(),
        }
    );
}

#[test]
fn worktree_create_full_upstream_payload() {
    let adapter = ClaudeAdapter;
    let input = json!({
        "session_id": "sess-1",
        "transcript_path": "/tmp/transcript",
        "cwd": "/home/user/project",
        "hook_event_name": "WorktreeCreate",
        "agent_id": "sub-1"
    });
    assert_eq!(
        adapter.parse("worktree-create", &input).unwrap(),
        AgentEvent::WorktreeCreate
    );
}

#[test]
fn worktree_remove_full_upstream_payload() {
    let adapter = ClaudeAdapter;
    let input = json!({
        "session_id": "sess-1",
        "transcript_path": "/tmp/transcript",
        "cwd": "/home/user/project",
        "hook_event_name": "WorktreeRemove",
        "worktree_path": "/tmp/worktrees/feat-branch",
        "agent_id": "sub-1"
    });
    let event = adapter.parse("worktree-remove", &input).unwrap();
    assert_eq!(
        event,
        AgentEvent::WorktreeRemove {
            worktree_path: "/tmp/worktrees/feat-branch".into(),
        }
    );
}

#[test]
fn stop_failure_full_upstream_payload() {
    let adapter = ClaudeAdapter;
    let input = json!({
        "session_id": "sess-1",
        "transcript_path": "/tmp/transcript",
        "cwd": "/home/user/project",
        "permission_mode": "auto",
        "hook_event_name": "StopFailure",
        "error_type": "rate_limit",
        "error_message": "Rate limit exceeded, please retry in 30s"
    });
    let event = adapter.parse("stop-failure", &input).unwrap();
    match event {
        AgentEvent::StopFailure { error, .. } => assert_eq!(error, "rate_limit"),
        other => panic!("expected StopFailure, got {:?}", other),
    }
}

#[test]
fn unknown_event_ignored() {
    let adapter = ClaudeAdapter;
    assert!(adapter.parse("unknown-event", &json!({})).is_none());
}

#[test]
fn subagent_stop_empty_type_ignored() {
    let adapter = ClaudeAdapter;
    assert!(adapter.parse("subagent-stop", &json!({})).is_none());
}

#[test]
fn notification_empty_reason() {
    let adapter = ClaudeAdapter;
    let input = json!({"cwd": "/tmp", "permission_mode": "default"});
    let event = adapter.parse("notification", &input).unwrap();
    assert_eq!(
        event,
        AgentEvent::Notification {
            agent: "claude".into(),
            cwd: "/tmp".into(),
            permission_mode: "default".into(),
            wait_reason: "".into(),
            meta_only: false,
            worktree: None,
            agent_id: None,
            session_id: None,
        }
    );
}

#[test]
fn stop_failure_legacy_error_beats_error_message() {
    let adapter = ClaudeAdapter;
    let input = json!({
        "cwd": "/tmp",
        "permission_mode": "default",
        "error": "legacy_wins",
        "error_message": "should_not_win"
    });
    let event = adapter.parse("stop-failure", &input).unwrap();
    match event {
        AgentEvent::StopFailure { error, .. } => assert_eq!(error, "legacy_wins"),
        other => panic!("expected StopFailure, got {:?}", other),
    }
}

#[test]
fn stop_failure_error_message_beats_error_details() {
    let adapter = ClaudeAdapter;
    let input = json!({
        "cwd": "/tmp",
        "permission_mode": "default",
        "error_message": "msg_wins",
        "error_details": "should_not_win"
    });
    let event = adapter.parse("stop-failure", &input).unwrap();
    match event {
        AgentEvent::StopFailure { error, .. } => assert_eq!(error, "msg_wins"),
        other => panic!("expected StopFailure, got {:?}", other),
    }
}

#[test]
fn stop_failure_error_type_takes_priority_over_legacy() {
    let adapter = ClaudeAdapter;
    let input = json!({
        "cwd": "/tmp",
        "permission_mode": "default",
        "error_type": "rate_limit",
        "error": "legacy_error",
        "error_message": "detail msg",
        "error_details": "legacy detail"
    });
    let event = adapter.parse("stop-failure", &input).unwrap();
    assert_eq!(
        event,
        AgentEvent::StopFailure {
            agent: "claude".into(),
            cwd: "/tmp".into(),
            permission_mode: "default".into(),
            error: "rate_limit".into(),
            worktree: None,
            agent_id: None,
            session_id: None,
        }
    );
}

#[test]
fn stop_failure_both_empty() {
    let adapter = ClaudeAdapter;
    let input = json!({"cwd": "/tmp", "permission_mode": "default"});
    let event = adapter.parse("stop-failure", &input).unwrap();
    assert_eq!(
        event,
        AgentEvent::StopFailure {
            agent: "claude".into(),
            cwd: "/tmp".into(),
            permission_mode: "default".into(),
            error: "".into(),
            worktree: None,
            agent_id: None,
            session_id: None,
        }
    );
}

#[test]
fn stop_empty_last_message() {
    let adapter = ClaudeAdapter;
    let input = json!({"cwd": "/tmp", "permission_mode": "default"});
    let event = adapter.parse("stop", &input).unwrap();
    assert_eq!(
        event,
        AgentEvent::Stop {
            agent: "claude".into(),
            cwd: "/tmp".into(),
            permission_mode: "default".into(),
            last_message: "".into(),
            response: None,
            worktree: None,
            agent_id: None,
            session_id: None,
        }
    );
}

#[test]
fn session_start_with_worktree_and_agent_id() {
    let adapter = ClaudeAdapter;
    let input = json!({
        "cwd": "/tmp/wt",
        "permission_mode": "auto",
        "agent_id": "abc-123",
        "worktree": {
            "name": "feat-wt",
            "path": "/tmp/wt",
            "branch": "feat",
            "originalRepoDir": "/home/user/repo"
        }
    });
    let event = adapter.parse("session-start", &input).unwrap();
    match event {
        AgentEvent::SessionStart {
            worktree, agent_id, ..
        } => {
            let wt = worktree.unwrap();
            assert_eq!(wt.name, "feat-wt");
            assert_eq!(wt.path, "/tmp/wt");
            assert_eq!(wt.branch, "feat");
            assert_eq!(wt.original_repo_dir, "/home/user/repo");
            assert_eq!(agent_id.unwrap(), "abc-123");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn session_start_without_worktree_fields() {
    let adapter = ClaudeAdapter;
    let input = json!({"cwd": "/tmp", "permission_mode": "default"});
    let event = adapter.parse("session-start", &input).unwrap();
    match event {
        AgentEvent::SessionStart {
            worktree, agent_id, ..
        } => {
            assert!(worktree.is_none());
            assert!(agent_id.is_none());
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn permission_denied_event() {
    let adapter = ClaudeAdapter;
    let input = json!({
        "cwd": "/tmp",
        "permission_mode": "auto",
        "tool_name": "Bash",
    });
    let event = adapter.parse("permission-denied", &input).unwrap();
    assert_eq!(
        event,
        AgentEvent::PermissionDenied {
            agent: "claude".into(),
            cwd: "/tmp".into(),
            permission_mode: "auto".into(),
            worktree: None,
            agent_id: None,
            session_id: None,
        }
    );
}

#[test]
fn cwd_changed_event() {
    let adapter = ClaudeAdapter;
    let input = json!({"cwd": "/new/path"});
    let event = adapter.parse("cwd-changed", &input).unwrap();
    assert_eq!(
        event,
        AgentEvent::CwdChanged {
            cwd: "/new/path".into(),
            worktree: None,
            agent_id: None,
            session_id: None,
        }
    );
}

#[test]
fn cwd_changed_with_worktree() {
    let adapter = ClaudeAdapter;
    let input = json!({
        "cwd": "/tmp/wt/src",
        "worktree": {
            "name": "wt",
            "path": "/tmp/wt",
            "branch": "main",
            "originalRepoDir": "/home/user/repo"
        }
    });
    let event = adapter.parse("cwd-changed", &input).unwrap();
    match event {
        AgentEvent::CwdChanged { cwd, worktree, .. } => {
            assert_eq!(cwd, "/tmp/wt/src");
            let wt = worktree.unwrap();
            assert_eq!(wt.original_repo_dir, "/home/user/repo");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn parse_worktree_empty_object_returns_none() {
    let adapter = ClaudeAdapter;
    let input = json!({"cwd": "/tmp", "permission_mode": "default", "worktree": {}});
    let event = adapter.parse("session-start", &input).unwrap();
    match event {
        AgentEvent::SessionStart { worktree, .. } => {
            assert!(worktree.is_none(), "empty worktree object should be None");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn parse_worktree_non_object_returns_none() {
    let adapter = ClaudeAdapter;
    let input = json!({"cwd": "/tmp", "permission_mode": "default", "worktree": "not-an-object"});
    let event = adapter.parse("session-start", &input).unwrap();
    match event {
        AgentEvent::SessionStart { worktree, .. } => {
            assert!(worktree.is_none(), "non-object worktree should be None");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn session_start_missing_fields_default_to_empty() {
    let adapter = ClaudeAdapter;
    let event = adapter.parse("session-start", &json!({})).unwrap();
    assert_eq!(
        event,
        AgentEvent::SessionStart {
            agent: "claude".into(),
            cwd: "".into(),
            permission_mode: "".into(),
            worktree: None,
            agent_id: None,
            session_id: None,
        }
    );
}

#[test]
fn activity_log_with_tool_response() {
    let adapter = ClaudeAdapter;
    let input = json!({
        "tool_name": "TaskCreate",
        "tool_input": {"subject": "Fix bug"},
        "tool_response": {"task": {"id": "42"}}
    });
    let event = adapter.parse("activity-log", &input).unwrap();
    assert_eq!(
        event,
        AgentEvent::ActivityLog {
            tool_name: "TaskCreate".into(),
            tool_input: json!({"subject": "Fix bug"}),
            tool_response: json!({"task": {"id": "42"}}),
        }
    );
}
