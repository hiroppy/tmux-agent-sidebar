# Event Adapter Implementation Plan

> **Status: Completed.** All 6 tasks shipped in `src/event.rs`, `src/adapter/{mod,claude,codex}.rs`, and `src/cli/hook.rs`. The `- [ ]` checkboxes below are kept as a historical record of the original plan steps. See the **Post-Plan Extensions** section below for what grew beyond the original design.

**Goal:** Decouple hook event handling from agent-specific event names/JSON schemas by introducing an internal `AgentEvent` enum and per-agent adapter modules.

**Architecture:** External events from each agent (Claude, Codex, future Gemini) flow through an adapter that maps them to a shared `AgentEvent` enum with all fields pre-extracted. The core handler (`handle_event`) only operates on `AgentEvent` — no agent names, no raw JSON.

**Tech Stack:** Rust, serde_json

**Spec:** `docs/specs/2026-04-06-event-adapter-design.md`
**Mapping reference:** `docs/event-mapping.md`

---

## File Structure

```
src/
  lib.rs                    — add `mod event; mod adapter;`
  event.rs                  — NEW: AgentEvent enum, EventAdapter trait, resolve_adapter()
  adapter/
    mod.rs                  — NEW: re-exports
    claude.rs               — NEW: ClaudeAdapter
    codex.rs                — NEW: CodexAdapter
  cli/
    hook.rs                 — MODIFY: replace match-on-strings with adapter + handle_event
    mod.rs                  — no changes (json_str, parse_json_field stay here as shared helpers)
```

Key decisions:
- `json_str`, `parse_json_field`, `sanitize_tmux_value` stay in `src/cli/mod.rs` — they are shared CLI helpers
- `set_agent_meta`, `set_status`, `set_attention`, `clear_run_state`, `clear_all_meta`, `is_system_message`, `append_subagent`, `remove_last_subagent`, `write_activity_entry`, `trim_log_file`, `should_update_cwd` stay in `src/cli/hook.rs` — they are handler internals
- `extract_tool_label` stays in `src/cli/label.rs`
- Adapters live in `src/adapter/` (sibling to `src/cli/`), trait + enum in `src/event.rs`

---

### Task 1: Create `AgentEvent` enum and `EventAdapter` trait

**Files:**
- Create: `src/event.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create `src/event.rs` with enum and trait**

```rust
use serde_json::Value;

/// Internal event representation. All fields are pre-extracted by the adapter.
/// The core handler never reads raw JSON or checks agent names.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentEvent {
    SessionStart {
        agent: String,
        cwd: String,
        permission_mode: String,
    },
    SessionEnd,
    UserPromptSubmit {
        agent: String,
        cwd: String,
        permission_mode: String,
        prompt: String,
    },
    Notification {
        agent: String,
        cwd: String,
        permission_mode: String,
        wait_reason: String,
    },
    Stop {
        agent: String,
        cwd: String,
        permission_mode: String,
        last_message: String,
        response: Option<String>,
    },
    StopFailure {
        agent: String,
        cwd: String,
        permission_mode: String,
        error: String,
    },
    SubagentStart {
        agent_type: String,
    },
    SubagentStop {
        agent_type: String,
    },
    ActivityLog {
        tool_name: String,
        tool_input: Value,
        tool_response: Value,
    },
}

/// Adapter that converts external agent events into internal `AgentEvent`.
pub trait EventAdapter {
    fn parse(&self, event_name: &str, input: &Value) -> Option<AgentEvent>;
}
```

- [ ] **Step 2: Register the module in `src/lib.rs`**

Add `pub mod event;` to `src/lib.rs` (after `pub mod cli;`).

- [ ] **Step 3: Run `cargo check`**

Run: `cargo check`
Expected: compiles successfully

- [ ] **Step 4: Commit**

```bash
git add src/event.rs src/lib.rs
git commit -m "feat: add AgentEvent enum and EventAdapter trait"
```

---

### Task 2: Create Claude adapter

**Files:**
- Create: `src/adapter/mod.rs`
- Create: `src/adapter/claude.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write adapter tests in `src/adapter/claude.rs`**

```rust
use crate::event::{AgentEvent, EventAdapter};
use serde_json::Value;

fn json_str<'a>(val: &'a Value, key: &str) -> &'a str {
    val.get(key).and_then(|v| v.as_str()).unwrap_or("")
}

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

impl EventAdapter for ClaudeAdapter {
    fn parse(&self, event_name: &str, input: &Value) -> Option<AgentEvent> {
        match event_name {
            "session-start" => Some(AgentEvent::SessionStart {
                agent: "claude".into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: json_str(input, "permission_mode").into(),
            }),
            "session-end" => Some(AgentEvent::SessionEnd),
            "user-prompt-submit" => Some(AgentEvent::UserPromptSubmit {
                agent: "claude".into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: json_str(input, "permission_mode").into(),
                prompt: json_str(input, "prompt").into(),
            }),
            "notification" => {
                let wait_reason = json_str(input, "notification_type");
                if wait_reason == "idle_prompt" {
                    return None;
                }
                Some(AgentEvent::Notification {
                    agent: "claude".into(),
                    cwd: json_str(input, "cwd").into(),
                    permission_mode: json_str(input, "permission_mode").into(),
                    wait_reason: wait_reason.into(),
                })
            }
            "stop" => Some(AgentEvent::Stop {
                agent: "claude".into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: json_str(input, "permission_mode").into(),
                last_message: json_str(input, "last_assistant_message").into(),
                response: None,
            }),
            "stop-failure" => {
                let error_type = json_str(input, "error");
                let error_details = json_str(input, "error_details");
                let error = if !error_type.is_empty() {
                    error_type
                } else {
                    error_details
                };
                Some(AgentEvent::StopFailure {
                    agent: "claude".into(),
                    cwd: json_str(input, "cwd").into(),
                    permission_mode: json_str(input, "permission_mode").into(),
                    error: error.into(),
                })
            }
            "subagent-start" => {
                let agent_type = json_str(input, "agent_type");
                if agent_type.is_empty() {
                    return None;
                }
                Some(AgentEvent::SubagentStart {
                    agent_type: agent_type.into(),
                })
            }
            "subagent-stop" => {
                let agent_type = json_str(input, "agent_type");
                if agent_type.is_empty() {
                    return None;
                }
                Some(AgentEvent::SubagentStop {
                    agent_type: agent_type.into(),
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
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
            }
        );
    }

    #[test]
    fn session_end() {
        let adapter = ClaudeAdapter;
        let event = adapter.parse("session-end", &json!({})).unwrap();
        assert_eq!(event, AgentEvent::SessionEnd);
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
            }
        );
    }

    #[test]
    fn notification() {
        let adapter = ClaudeAdapter;
        let input = json!({"cwd": "/tmp", "permission_mode": "default", "notification_type": "permission"});
        let event = adapter.parse("notification", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::Notification {
                agent: "claude".into(),
                cwd: "/tmp".into(),
                permission_mode: "default".into(),
                wait_reason: "permission".into(),
            }
        );
    }

    #[test]
    fn notification_idle_prompt_ignored() {
        let adapter = ClaudeAdapter;
        let input = json!({"notification_type": "idle_prompt"});
        assert!(adapter.parse("notification", &input).is_none());
    }

    #[test]
    fn stop() {
        let adapter = ClaudeAdapter;
        let input = json!({"cwd": "/tmp", "permission_mode": "default", "last_assistant_message": "done"});
        let event = adapter.parse("stop", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::Stop {
                agent: "claude".into(),
                cwd: "/tmp".into(),
                permission_mode: "default".into(),
                last_message: "done".into(),
                response: None,
            }
        );
    }

    #[test]
    fn stop_failure_error_field() {
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
            }
        );
    }

    #[test]
    fn subagent_start() {
        let adapter = ClaudeAdapter;
        let input = json!({"agent_type": "Explore"});
        let event = adapter.parse("subagent-start", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::SubagentStart {
                agent_type: "Explore".into(),
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
        let event = adapter.parse("subagent-stop", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::SubagentStop {
                agent_type: "Plan".into(),
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
    fn activity_log_empty_tool_name_ignored() {
        let adapter = ClaudeAdapter;
        assert!(adapter.parse("activity-log", &json!({})).is_none());
    }

    #[test]
    fn unknown_event_ignored() {
        let adapter = ClaudeAdapter;
        assert!(adapter.parse("unknown-event", &json!({})).is_none());
    }
}
```

- [ ] **Step 2: Create `src/adapter/mod.rs`**

```rust
pub mod claude;
pub mod codex;
```

- [ ] **Step 3: Register module in `src/lib.rs`**

Add `pub mod adapter;` to `src/lib.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib adapter::claude`
Expected: all tests pass

- [ ] **Step 5: Commit**

```bash
git add src/adapter/ src/lib.rs
git commit -m "feat: add Claude event adapter"
```

---

### Task 3: Create Codex adapter

**Files:**
- Create: `src/adapter/codex.rs`

- [ ] **Step 1: Write `src/adapter/codex.rs`**

```rust
use crate::event::{AgentEvent, EventAdapter};
use serde_json::Value;

fn json_str<'a>(val: &'a Value, key: &str) -> &'a str {
    val.get(key).and_then(|v| v.as_str()).unwrap_or("")
}

pub struct CodexAdapter;

impl EventAdapter for CodexAdapter {
    fn parse(&self, event_name: &str, input: &Value) -> Option<AgentEvent> {
        match event_name {
            "session-start" => Some(AgentEvent::SessionStart {
                agent: "codex".into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: json_str(input, "permission_mode").into(),
            }),
            "session-end" => Some(AgentEvent::SessionEnd),
            "user-prompt-submit" => Some(AgentEvent::UserPromptSubmit {
                agent: "codex".into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: json_str(input, "permission_mode").into(),
                prompt: json_str(input, "prompt").into(),
            }),
            "stop" => Some(AgentEvent::Stop {
                agent: "codex".into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: json_str(input, "permission_mode").into(),
                last_message: json_str(input, "last_assistant_message").into(),
                response: Some("{\"continue\":true}".into()),
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn session_start() {
        let adapter = CodexAdapter;
        let input = json!({"cwd": "/home/user"});
        let event = adapter.parse("session-start", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionStart {
                agent: "codex".into(),
                cwd: "/home/user".into(),
                permission_mode: "".into(),
            }
        );
    }

    #[test]
    fn session_end() {
        let adapter = CodexAdapter;
        let event = adapter.parse("session-end", &json!({})).unwrap();
        assert_eq!(event, AgentEvent::SessionEnd);
    }

    #[test]
    fn user_prompt_submit() {
        let adapter = CodexAdapter;
        let input = json!({"cwd": "/tmp", "prompt": "hello"});
        let event = adapter.parse("user-prompt-submit", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::UserPromptSubmit {
                agent: "codex".into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                prompt: "hello".into(),
            }
        );
    }

    #[test]
    fn stop_has_continue_response() {
        let adapter = CodexAdapter;
        let input = json!({"cwd": "/tmp", "last_assistant_message": "done"});
        let event = adapter.parse("stop", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::Stop {
                agent: "codex".into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                last_message: "done".into(),
                response: Some("{\"continue\":true}".into()),
            }
        );
    }

    #[test]
    fn notification_not_supported() {
        let adapter = CodexAdapter;
        assert!(adapter.parse("notification", &json!({})).is_none());
    }

    #[test]
    fn stop_failure_not_supported() {
        let adapter = CodexAdapter;
        assert!(adapter.parse("stop-failure", &json!({})).is_none());
    }

    #[test]
    fn subagent_start_not_supported() {
        let adapter = CodexAdapter;
        assert!(adapter.parse("subagent-start", &json!({})).is_none());
    }

    #[test]
    fn activity_log_not_supported() {
        let adapter = CodexAdapter;
        assert!(adapter.parse("activity-log", &json!({})).is_none());
    }

    #[test]
    fn unknown_event_ignored() {
        let adapter = CodexAdapter;
        assert!(adapter.parse("something-else", &json!({})).is_none());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib adapter::codex`
Expected: all tests pass

- [ ] **Step 3: Commit**

```bash
git add src/adapter/codex.rs
git commit -m "feat: add Codex event adapter"
```

---

### Task 4: Add `resolve_adapter` to `src/event.rs`

**Files:**
- Modify: `src/event.rs`

- [ ] **Step 1: Add `resolve_adapter` function and tests**

Append to `src/event.rs`:

```rust
use crate::adapter;

pub fn resolve_adapter(agent_name: &str) -> Option<Box<dyn EventAdapter>> {
    match agent_name {
        "claude" => Some(Box::new(adapter::claude::ClaudeAdapter)),
        "codex" => Some(Box::new(adapter::codex::CodexAdapter)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolve_claude() {
        let adapter = resolve_adapter("claude");
        assert!(adapter.is_some());
        let event = adapter.unwrap().parse("session-end", &json!({}));
        assert_eq!(event, Some(AgentEvent::SessionEnd));
    }

    #[test]
    fn resolve_codex() {
        let adapter = resolve_adapter("codex");
        assert!(adapter.is_some());
    }

    #[test]
    fn resolve_unknown_returns_none() {
        assert!(resolve_adapter("gemini").is_none());
        assert!(resolve_adapter("").is_none());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib event`
Expected: all tests pass

- [ ] **Step 3: Commit**

```bash
git add src/event.rs
git commit -m "feat: add resolve_adapter for agent name lookup"
```

---

### Task 5: Refactor `cmd_hook` to use adapter + `handle_event`

**Files:**
- Modify: `src/cli/hook.rs`

This is the main refactor. The existing `match event` block becomes `handle_event` operating on `AgentEvent`. The `json_str` calls in handler arms are replaced by pre-extracted fields from the enum.

- [ ] **Step 1: Add `handle_event` function to `src/cli/hook.rs`**

Replace the body of `cmd_hook` and extract `handle_event`. Keep all helper functions (`set_agent_meta`, `clear_run_state`, etc.) as-is, but `set_agent_meta` now takes individual fields instead of raw JSON:

```rust
use crate::event::{resolve_adapter, AgentEvent};

/// Apply agent metadata (agent name, cwd, permission_mode) to tmux pane options.
fn set_agent_meta(pane: &str, agent: &str, cwd: &str, permission_mode: &str) {
    tmux::set_pane_option(pane, "@pane_agent", agent);
    if !cwd.is_empty() {
        let current_subagents = tmux::get_pane_option_value(pane, "@pane_subagents");
        if should_update_cwd(&current_subagents) {
            tmux::set_pane_option(pane, "@pane_cwd", cwd);
        }
    }
    if !permission_mode.is_empty() {
        tmux::set_pane_option(pane, "@pane_permission_mode", permission_mode);
    }
}
```

New `cmd_hook`:

```rust
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
```

New `handle_event`:

```rust
fn handle_event(pane: &str, event: AgentEvent) -> i32 {
    match event {
        AgentEvent::SessionStart {
            agent,
            cwd,
            permission_mode,
        } => {
            set_agent_meta(pane, &agent, &cwd, &permission_mode);
            set_attention(pane, "clear");
            clear_run_state(pane);
            tmux::unset_pane_option(pane, "@pane_prompt");
            tmux::unset_pane_option(pane, "@pane_prompt_source");
            tmux::unset_pane_option(pane, "@pane_subagents");
            set_status(pane, "idle");
        }
        AgentEvent::SessionEnd => {
            set_attention(pane, "clear");
            clear_all_meta(pane);
            set_status(pane, "clear");
            let log_path = crate::activity::log_file_path(pane);
            let _ = std::fs::remove_file(log_path);
        }
        AgentEvent::UserPromptSubmit {
            agent,
            cwd,
            permission_mode,
            prompt,
        } => {
            set_agent_meta(pane, &agent, &cwd, &permission_mode);
            set_attention(pane, "clear");
            set_status(pane, "running");
            if !prompt.is_empty() && !is_system_message(&prompt) {
                let p = sanitize_tmux_value(&prompt);
                tmux::set_pane_option(pane, "@pane_prompt", &p);
                tmux::set_pane_option(pane, "@pane_prompt_source", "user");
            }
            let now = unsafe { libc::time(std::ptr::null_mut()) };
            tmux::set_pane_option(pane, "@pane_started_at", &now.to_string());
            tmux::unset_pane_option(pane, "@pane_wait_reason");
        }
        AgentEvent::Notification {
            agent,
            cwd,
            permission_mode,
            wait_reason,
        } => {
            set_agent_meta(pane, &agent, &cwd, &permission_mode);
            set_status(pane, "waiting");
            set_attention(pane, "notification");
            if !wait_reason.is_empty() {
                tmux::set_pane_option(pane, "@pane_wait_reason", &wait_reason);
            }
        }
        AgentEvent::Stop {
            agent,
            cwd,
            permission_mode,
            last_message,
            response,
        } => {
            set_agent_meta(pane, &agent, &cwd, &permission_mode);
            set_attention(pane, "clear");
            if !last_message.is_empty() {
                let msg = sanitize_tmux_value(&last_message);
                tmux::set_pane_option(pane, "@pane_prompt", &msg);
                tmux::set_pane_option(pane, "@pane_prompt_source", "response");
            }
            clear_run_state(pane);
            set_status(pane, "idle");
            if let Some(resp) = response {
                println!("{resp}");
            }
        }
        AgentEvent::StopFailure {
            agent,
            cwd,
            permission_mode,
            error,
        } => {
            set_agent_meta(pane, &agent, &cwd, &permission_mode);
            set_attention(pane, "clear");
            clear_run_state(pane);
            if !error.is_empty() {
                tmux::set_pane_option(pane, "@pane_wait_reason", &error);
            }
            set_status(pane, "error");
        }
        AgentEvent::SubagentStart { agent_type } => {
            let current = tmux::get_pane_option_value(pane, "@pane_subagents");
            let new_val = append_subagent(&current, &agent_type);
            tmux::set_pane_option(pane, "@pane_subagents", &new_val);
        }
        AgentEvent::SubagentStop { agent_type } => {
            let current = tmux::get_pane_option_value(pane, "@pane_subagents");
            match remove_last_subagent(&current, &agent_type) {
                None => return 0,
                Some(new_val) if new_val.is_empty() => {
                    tmux::unset_pane_option(pane, "@pane_subagents");
                }
                Some(new_val) => {
                    tmux::set_pane_option(pane, "@pane_subagents", &new_val);
                }
            }
        }
        AgentEvent::ActivityLog {
            tool_name,
            tool_input,
            tool_response,
        } => {
            return handle_activity_log(pane, &tool_name, &tool_input, &tool_response);
        }
    }
    0
}
```

Also update `handle_activity_log` signature to take pre-extracted fields:

```rust
fn handle_activity_log(pane: &str, tool_name: &str, tool_input: &serde_json::Value, tool_response: &serde_json::Value) -> i32 {
    let label = extract_tool_label(tool_name, tool_input, tool_response);

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
```

- [ ] **Step 2: Remove old `set_agent_meta` that takes `&serde_json::Value`**

The old signature `fn set_agent_meta(pane: &str, agent: &str, json: &serde_json::Value)` is replaced by the new one above. Delete the old version entirely.

- [ ] **Step 3: Run all tests**

Run: `cargo test`
Expected: all tests pass. Existing tests in `hook.rs::tests` (append_subagent, remove_last_subagent, parse_json_field, trim_log_file, write_activity_entry, is_system_message, should_update_cwd) still pass. The `handle_activity_log` tests still pass since the function signature change is compatible.

Note: `parse_json_field` tests in `hook.rs` can stay — the function is still used as a helper in `hook.rs` tests but the adapters have their own copy for field extraction. If `parse_json_field` is no longer called from `hook.rs` handler code, remove it and its tests from `hook.rs` (it lives in the Claude adapter now). Keep the tests that test `handle_activity_log` directly — those now call with pre-extracted values instead of raw JSON.

- [ ] **Step 4: Run clippy**

Run: `cargo clippy`
Expected: no warnings

- [ ] **Step 5: Run fmt**

Run: `cargo fmt`

- [ ] **Step 6: Commit**

```bash
git add src/cli/hook.rs
git commit -m "refactor: use event adapter pattern in hook handler"
```

---

### Task 6: Clean up dead code

**Files:**
- Modify: `src/cli/hook.rs`
- Modify: `src/cli/mod.rs` (if needed)

- [ ] **Step 1: Remove unused imports and functions from `hook.rs`**

After the refactor, check what's no longer needed in `hook.rs`:
- `json_str` import from `super` — no longer used in handler (adapters extract fields). Keep if tests still use `json_str` from `super`.
- `parse_json_field` — moved into adapters. Remove from `hook.rs` if no longer called. Move its tests to the adapter that uses it (Claude adapter already has its own implementation).

- [ ] **Step 2: Run tests + clippy + fmt**

Run: `cargo test && cargo clippy && cargo fmt --check`
Expected: all pass, no warnings, no format issues

- [ ] **Step 3: Commit**

```bash
git add src/cli/hook.rs src/cli/mod.rs
git commit -m "chore: remove dead code after adapter refactor"
```

---

## Verification

After all tasks:

1. `cargo test` — all existing + new tests pass
2. `cargo clippy` — no warnings
3. `cargo fmt --check` — clean
4. Manual check: `grep -r '"claude"\|"codex"' src/cli/hook.rs` returns no matches (handler is agent-agnostic)
5. Manual check: `grep -r 'json_str' src/cli/hook.rs` returns no matches in non-test code (handler doesn't read raw JSON)

---

## Post-Plan Extensions

The shipped implementation grew beyond the 9 variants in this plan. These were added incrementally after the initial refactor landed and are documented here so the historical plan does not imply a narrower contract than what actually exists.

### Additional `AgentEvent` variants (source of truth: `src/event.rs`)

- `PermissionDenied { agent, cwd, permission_mode, worktree, agent_id, session_id }` — Claude-only.
- `CwdChanged { cwd, worktree, agent_id, session_id }` — fires when the agent reports a `chdir`.
- `TaskCreated { task_id, task_subject }` / `TaskCompleted { task_id, task_subject }` — task lifecycle surfaced in the bottom panel.
- `TeammateIdle { teammate_name, team_name }` — team coordination signal.
- `WorktreeCreate` (unit) / `WorktreeRemove { worktree_path }` — worktree lifecycle.

### Additional fields on existing variants

- `SessionStart`, `UserPromptSubmit`, `Notification`, `Stop`, `StopFailure`, `PermissionDenied`, `CwdChanged` all carry `worktree: Option<WorktreeInfo>`, `agent_id: Option<String>`, `session_id: Option<String>`.
- `Notification` also carries `meta_only: bool` so events like `idle_prompt` can refresh metadata without flipping the visible status.
- `SubagentStart { agent_type, agent_id }` — agent_id added.
- `SubagentStop { agent_type, agent_id, last_message, transcript_path }` — three new fields so the handler can surface the subagent's final message and link back to its transcript.
- New sibling struct `WorktreeInfo { name, path, branch, original_repo_dir }`.

### `AgentEventKind` + `HOOK_REGISTRATIONS`

A discriminant enum `AgentEventKind` was added alongside `AgentEvent` (16 variants) with `ALL`, `external_name()`, and `from_external_name()`. Each adapter now exposes a `HOOK_REGISTRATIONS: &'static [HookRegistration]` table (`src/adapter/mod.rs`) that binds upstream trigger names + matchers to `AgentEventKind`. A shared `assert_table_drift_free` test guarantees the table and `parse()` match arms stay in sync. This is the single source of truth consumed by the `setup` subcommand (`src/cli/setup.rs`) that prints ready-to-paste hook configs — no hook identity is duplicated anywhere.

### Handler-side

- `handle_event` in `src/cli/hook.rs` writes an additional `@pane_session_id` tmux option via `sync_pane_location`, and resolves cwd through `resolve_cwd()` so worktree panes group under `original_repo_dir` instead of the worktree path.
- `clear_all_meta` clears `@pane_session_id` alongside the other `@pane_*` keys.
