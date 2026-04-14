use crate::event::{AgentEvent, AgentEventKind, EventAdapter, WorktreeInfo};
use crate::tmux::CLAUDE_AGENT;
use serde_json::Value;

use super::{HookRegistration, json_str};

/// Parse optional worktree object from hook payload.
/// Returns None if the "worktree" field is missing or not an object.
fn parse_worktree(input: &Value) -> Option<WorktreeInfo> {
    let obj = input.get("worktree")?;
    if !obj.is_object() {
        return None;
    }
    let name = json_str(obj, "name");
    let path = json_str(obj, "path");
    let branch = json_str(obj, "branch");
    let original = json_str(obj, "originalRepoDir");
    if name.is_empty() && path.is_empty() && branch.is_empty() && original.is_empty() {
        return None;
    }
    Some(WorktreeInfo {
        name: name.into(),
        path: path.into(),
        branch: branch.into(),
        original_repo_dir: original.into(),
    })
}

use super::optional_str;

fn parse_json_field(input: &Value, field: &str) -> Value {
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
        .unwrap_or(Value::Null)
}

pub struct ClaudeAdapter;

impl ClaudeAdapter {
    /// Single source of truth for Claude Code hook wiring. Each entry pairs
    /// a real Claude Code trigger (verified against the official hooks
    /// reference at code.claude.com/docs/en/hooks) with the internal
    /// `AgentEventKind` the sidebar produces. Drift against `parse()` is
    /// caught by `hook_registrations_match_parse_arms` below.
    ///
    /// Note: `PostToolUse` maps to `AgentEventKind::ActivityLog` — the only
    /// entry where the upstream trigger and the internal kind have
    /// different names.
    pub const HOOK_REGISTRATIONS: &'static [HookRegistration] = &[
        HookRegistration {
            trigger: "SessionStart",
            matcher: None,
            kind: AgentEventKind::SessionStart,
        },
        HookRegistration {
            trigger: "SessionEnd",
            matcher: None,
            kind: AgentEventKind::SessionEnd,
        },
        HookRegistration {
            trigger: "UserPromptSubmit",
            matcher: None,
            kind: AgentEventKind::UserPromptSubmit,
        },
        HookRegistration {
            trigger: "Notification",
            matcher: None,
            kind: AgentEventKind::Notification,
        },
        HookRegistration {
            trigger: "Stop",
            matcher: None,
            kind: AgentEventKind::Stop,
        },
        HookRegistration {
            trigger: "StopFailure",
            matcher: None,
            kind: AgentEventKind::StopFailure,
        },
        HookRegistration {
            trigger: "PermissionDenied",
            matcher: None,
            kind: AgentEventKind::PermissionDenied,
        },
        HookRegistration {
            trigger: "CwdChanged",
            matcher: None,
            kind: AgentEventKind::CwdChanged,
        },
        HookRegistration {
            trigger: "SubagentStart",
            matcher: None,
            kind: AgentEventKind::SubagentStart,
        },
        HookRegistration {
            trigger: "SubagentStop",
            matcher: None,
            kind: AgentEventKind::SubagentStop,
        },
        HookRegistration {
            trigger: "PostToolUse",
            matcher: None,
            kind: AgentEventKind::ActivityLog,
        },
        HookRegistration {
            trigger: "TaskCreated",
            matcher: None,
            kind: AgentEventKind::TaskCreated,
        },
        HookRegistration {
            trigger: "TaskCompleted",
            matcher: None,
            kind: AgentEventKind::TaskCompleted,
        },
        HookRegistration {
            trigger: "TeammateIdle",
            matcher: None,
            kind: AgentEventKind::TeammateIdle,
        },
        HookRegistration {
            trigger: "WorktreeCreate",
            matcher: None,
            kind: AgentEventKind::WorktreeCreate,
        },
        HookRegistration {
            trigger: "WorktreeRemove",
            matcher: None,
            kind: AgentEventKind::WorktreeRemove,
        },
    ];
}

impl EventAdapter for ClaudeAdapter {
    fn parse(&self, event_name: &str, input: &Value) -> Option<AgentEvent> {
        match event_name {
            "session-start" => Some(AgentEvent::SessionStart {
                agent: CLAUDE_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: json_str(input, "permission_mode").into(),
                worktree: parse_worktree(input),
                agent_id: optional_str(input, "agent_id"),
                session_id: optional_str(input, "session_id"),
            }),
            "session-end" => Some(AgentEvent::SessionEnd),
            "user-prompt-submit" => Some(AgentEvent::UserPromptSubmit {
                agent: CLAUDE_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: json_str(input, "permission_mode").into(),
                prompt: json_str(input, "prompt").into(),
                worktree: parse_worktree(input),
                agent_id: optional_str(input, "agent_id"),
                session_id: optional_str(input, "session_id"),
            }),
            "notification" => {
                let wait_reason = json_str(input, "notification_type");
                let meta_only = wait_reason == "idle_prompt";
                Some(AgentEvent::Notification {
                    agent: CLAUDE_AGENT.into(),
                    cwd: json_str(input, "cwd").into(),
                    permission_mode: json_str(input, "permission_mode").into(),
                    wait_reason: wait_reason.into(),
                    meta_only,
                    worktree: parse_worktree(input),
                    agent_id: optional_str(input, "agent_id"),
                    session_id: optional_str(input, "session_id"),
                })
            }
            "stop" => Some(AgentEvent::Stop {
                agent: CLAUDE_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: json_str(input, "permission_mode").into(),
                last_message: json_str(input, "last_assistant_message").into(),
                response: None,
                worktree: parse_worktree(input),
                agent_id: optional_str(input, "agent_id"),
                session_id: optional_str(input, "session_id"),
            }),
            "stop-failure" => {
                // Upstream fields: error_type (category), error_message (detail)
                // Legacy fields: error, error_details
                let error_type = json_str(input, "error_type");
                let error_legacy = json_str(input, "error");
                let error_message = json_str(input, "error_message");
                let error_details = json_str(input, "error_details");
                let error = if !error_type.is_empty() {
                    error_type
                } else if !error_legacy.is_empty() {
                    error_legacy
                } else if !error_message.is_empty() {
                    error_message
                } else {
                    error_details
                };
                Some(AgentEvent::StopFailure {
                    agent: CLAUDE_AGENT.into(),
                    cwd: json_str(input, "cwd").into(),
                    permission_mode: json_str(input, "permission_mode").into(),
                    error: error.into(),
                    worktree: parse_worktree(input),
                    agent_id: optional_str(input, "agent_id"),
                    session_id: optional_str(input, "session_id"),
                })
            }
            "permission-denied" => Some(AgentEvent::PermissionDenied {
                agent: CLAUDE_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: json_str(input, "permission_mode").into(),
                worktree: parse_worktree(input),
                agent_id: optional_str(input, "agent_id"),
                session_id: optional_str(input, "session_id"),
            }),
            "cwd-changed" => Some(AgentEvent::CwdChanged {
                cwd: json_str(input, "cwd").into(),
                worktree: parse_worktree(input),
                agent_id: optional_str(input, "agent_id"),
                session_id: optional_str(input, "session_id"),
            }),
            "subagent-start" => {
                let agent_type = json_str(input, "agent_type");
                if agent_type.is_empty() {
                    return None;
                }
                Some(AgentEvent::SubagentStart {
                    agent_type: agent_type.into(),
                    agent_id: optional_str(input, "agent_id"),
                })
            }
            "subagent-stop" => {
                let agent_type = json_str(input, "agent_type");
                if agent_type.is_empty() {
                    return None;
                }
                Some(AgentEvent::SubagentStop {
                    agent_type: agent_type.into(),
                    agent_id: optional_str(input, "agent_id"),
                    last_message: json_str(input, "last_assistant_message").into(),
                    transcript_path: json_str(input, "agent_transcript_path").into(),
                })
            }
            "activity-log" => {
                let tool_name = json_str(input, "tool_name");
                if tool_name.is_empty() {
                    return None;
                }
                Some(AgentEvent::ActivityLog {
                    tool_name: tool_name.into(),
                    tool_input: parse_json_field(input, "tool_input"),
                    tool_response: parse_json_field(input, "tool_response"),
                })
            }
            "task-created" => Some(AgentEvent::TaskCreated {
                task_id: json_str(input, "task_id").into(),
                task_subject: json_str(input, "task_subject").into(),
            }),
            "task-completed" => Some(AgentEvent::TaskCompleted {
                task_id: json_str(input, "task_id").into(),
                task_subject: json_str(input, "task_subject").into(),
            }),
            "teammate-idle" => Some(AgentEvent::TeammateIdle {
                teammate_name: json_str(input, "teammate_name").into(),
                team_name: json_str(input, "team_name").into(),
            }),
            "worktree-create" => Some(AgentEvent::WorktreeCreate),
            "worktree-remove" => Some(AgentEvent::WorktreeRemove {
                worktree_path: json_str(input, "worktree_path").into(),
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
