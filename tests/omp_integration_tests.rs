use serde_json::json;
use tmux_agent_sidebar::event::{AgentEvent, EventAdapter, resolve_adapter};
use tmux_agent_sidebar::tmux::{AgentType, OMP_AGENT};

fn omp_adapter() -> Box<dyn EventAdapter> {
    resolve_adapter(OMP_AGENT).expect("OMP must resolve to an event adapter")
}

#[test]
fn omp_label_round_trips_without_accepting_lookalike_process_labels() {
    let agent = AgentType::from_label(OMP_AGENT).expect("OMP label must be recognized");

    assert_eq!(agent, AgentType::Omp);
    assert_eq!(agent.as_str(), OMP_AGENT);
    assert_eq!(agent.label(), OMP_AGENT);
    assert_eq!(AgentType::from_label("not-omp"), None);
}

#[test]
fn resolved_omp_adapter_preserves_session_identity_cwd_and_prompt() {
    let adapter = omp_adapter();

    let started = adapter
        .parse(
            "session-start",
            &json!({
                "session_id": "omp-session-17",
                "cwd": "/worktrees/security-fix",
                "source": "startup"
            }),
        )
        .expect("OMP session-start must be accepted");
    match started {
        AgentEvent::SessionStart {
            agent,
            cwd,
            source,
            session_id,
            ..
        } => {
            assert_eq!(agent, OMP_AGENT);
            assert_eq!(cwd, "/worktrees/security-fix");
            assert_eq!(source, "startup");
            assert_eq!(session_id.as_deref(), Some("omp-session-17"));
        }
        other => panic!("expected SessionStart, got {other:?}"),
    }

    let prompted = adapter
        .parse(
            "user-prompt-submit",
            &json!({
                "session_id": "omp-session-17",
                "cwd": "/worktrees/security-fix",
                "prompt": "fix the authorization boundary"
            }),
        )
        .expect("OMP user-prompt-submit must be accepted");
    match prompted {
        AgentEvent::UserPromptSubmit {
            agent,
            cwd,
            prompt,
            session_id,
            ..
        } => {
            assert_eq!(agent, OMP_AGENT);
            assert_eq!(cwd, "/worktrees/security-fix");
            assert_eq!(prompt, "fix the authorization boundary");
            assert_eq!(session_id.as_deref(), Some("omp-session-17"));
        }
        other => panic!("expected UserPromptSubmit, got {other:?}"),
    }
}

#[test]
fn omp_stop_preserves_the_final_message_but_keeps_the_hook_passive() {
    let event = omp_adapter()
        .parse(
            "stop",
            &json!({
                "cwd": "/worktrees/security-fix",
                "session_id": "omp-session-17",
                "last_message": "Authorization boundary is fixed.",
            }),
        )
        .expect("OMP stop must be accepted");

    match event {
        AgentEvent::Stop {
            agent,
            cwd,
            last_message,
            response,
            session_id,
            ..
        } => {
            assert_eq!(agent, OMP_AGENT);
            assert_eq!(cwd, "/worktrees/security-fix");
            assert_eq!(last_message, "Authorization boundary is fixed.");
            assert_eq!(response, None);
            assert_eq!(session_id.as_deref(), Some("omp-session-17"));
        }
        other => panic!("expected Stop, got {other:?}"),
    }
}

#[test]
fn omp_tool_activity_records_the_started_tool_without_sensitive_arguments() {
    let event = omp_adapter()
        .parse(
            "activity-log",
            &json!({
                "cwd": "/worktrees/security-fix",
                "session_id": "omp-session-17",
                "tool_name": "Bash",
                "tool_input": {}
            }),
        )
        .expect("OMP activity-log must be accepted");

    assert_eq!(
        event,
        AgentEvent::ActivityLog {
            tool_name: "Bash".into(),
            tool_input: json!({}),
            tool_response: serde_json::Value::Null,
        }
    );
}

#[test]
fn omp_notification_and_session_end_reach_sidebar_state_transitions() {
    let adapter = omp_adapter();

    let notification = adapter
        .parse(
            "notification",
            &json!({
                "session_id": "omp-session-17",
                "cwd": "/worktrees/security-fix",
                "wait_reason": "permission"
            }),
        )
        .expect("OMP notification must be accepted");
    match notification {
        AgentEvent::Notification {
            agent,
            cwd,
            wait_reason,
            session_id,
            ..
        } => {
            assert_eq!(agent, OMP_AGENT);
            assert_eq!(cwd, "/worktrees/security-fix");
            assert_eq!(wait_reason, "permission");
            assert_eq!(session_id.as_deref(), Some("omp-session-17"));
        }
        other => panic!("expected Notification, got {other:?}"),
    }

    assert_eq!(
        adapter.parse(
            "session-end",
            &json!({
                "cwd": "/worktrees/security-fix",
                "session_id": "omp-session-17",
                "end_reason": "shutdown"
            })
        ),
        Some(AgentEvent::SessionEnd {
            end_reason: "shutdown".into()
        })
    );
}
