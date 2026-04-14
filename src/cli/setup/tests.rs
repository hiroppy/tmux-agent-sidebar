
use super::*;
use serde_json::{Value, json};

const FAKE_HOOK: &str = "/fake/hook.sh";

#[test]
fn shell_quote_safe_string_passes_through() {
    // Fast path: common paths have no shell-special characters and are
    // returned verbatim. This is what keeps the fallback
    // `~/.tmux/plugins/tmux-agent-sidebar/hook.sh` eligible for tilde
    // expansion in the generated command.
    assert_eq!(shell_quote("hello"), "hello");
    assert_eq!(shell_quote("/fake/hook.sh"), "/fake/hook.sh");
    assert_eq!(
        shell_quote("~/.tmux/plugins/tmux-agent-sidebar/hook.sh"),
        "~/.tmux/plugins/tmux-agent-sidebar/hook.sh"
    );
    assert_eq!(
        shell_quote("/Users/alice/.tmux/plugins/tmux-agent-sidebar/hook.sh"),
        "/Users/alice/.tmux/plugins/tmux-agent-sidebar/hook.sh"
    );
}

#[test]
fn shell_quote_empty_string_is_quoted() {
    // Empty arg must survive as `''`, otherwise it vanishes from argv.
    assert_eq!(shell_quote(""), "''");
}

#[test]
fn shell_quote_path_with_spaces() {
    assert_eq!(shell_quote("/Users/a b/hook.sh"), "'/Users/a b/hook.sh'");
}

#[test]
fn shell_quote_embedded_single_quote() {
    // POSIX trick: 'a'\''b' = literal `a'b`.
    assert_eq!(shell_quote("a'b"), "'a'\\''b'");
}

#[test]
fn shell_quote_shell_metacharacters() {
    // `$`, backticks, `;`, `|` must all be neutralized inside single quotes.
    assert_eq!(shell_quote("$(rm -rf /)"), "'$(rm -rf /)'");
    assert_eq!(shell_quote("a;b|c`d`"), "'a;b|c`d`'");
}

#[test]
fn format_hook_command_leaves_safe_path_unquoted() {
    let cmd = format_hook_command("/fake/hook.sh", "claude", "session-start");
    assert_eq!(cmd, "bash /fake/hook.sh claude session-start");
}

#[test]
fn format_hook_command_quotes_unsafe_path() {
    let cmd = format_hook_command("/path with space/hook.sh", "claude", "session-start");
    assert_eq!(cmd, "bash '/path with space/hook.sh' claude session-start");
}

#[test]
fn snippet_path_with_spaces_is_safely_quoted() {
    // Paths with spaces must survive the JSON round-trip as a quoted
    // single shell token. Before the fix, this produced
    // `bash /path with/hook.sh claude session-start` which `bash` would
    // parse as four arguments.
    let v = build_agent_snippet("claude", "/path with spaces/hook.sh").unwrap();
    let cmd = v
        .pointer("/hooks/SessionStart/0/hooks/0/command")
        .and_then(Value::as_str)
        .unwrap();
    assert_eq!(cmd, "bash '/path with spaces/hook.sh' claude session-start");
}

#[test]
fn snippet_path_with_single_quote_is_escaped() {
    let v = build_agent_snippet("claude", "/weird'path/hook.sh").unwrap();
    let cmd = v
        .pointer("/hooks/SessionStart/0/hooks/0/command")
        .and_then(Value::as_str)
        .unwrap();
    assert_eq!(cmd, "bash '/weird'\\''path/hook.sh' claude session-start");
}

#[test]
fn resolve_hook_script_fallback_when_binary_has_no_sibling() {
    // We cannot pin current_exe() in a unit test, but we can verify the
    // FALLBACK constant is what the resolver returns as its `path` field
    // when `detected = false`, by exercising the branch indirectly: any
    // `ResolvedHookScript` whose `detected` flag is false MUST use the
    // documented fallback string so cmd_setup's warning points somewhere
    // meaningful.
    let resolved = resolve_hook_script();
    if !resolved.detected {
        assert_eq!(resolved.path, FALLBACK_HOOK_SCRIPT);
    }
    // When detected is true, we at least verify the file exists on disk
    // (otherwise the resolver lied).
    if resolved.detected {
        assert!(std::path::Path::new(&resolved.path).is_file());
    }
}

#[test]
fn snippet_unknown_agent_returns_none() {
    assert!(build_agent_snippet("not-an-agent", FAKE_HOOK).is_none());
}

#[test]
fn snippet_claude_has_hooks_key() {
    let v = build_agent_snippet("claude", FAKE_HOOK).unwrap();
    assert!(v.get("hooks").is_some(), "missing top-level hooks key");
    assert!(v.get("hooks").unwrap().is_object());
}

#[test]
fn snippet_claude_covers_every_registration() {
    let v = build_agent_snippet("claude", FAKE_HOOK).unwrap();
    let hooks = v.get("hooks").unwrap().as_object().unwrap();
    let mut expected_triggers: Vec<&str> = ClaudeAdapter::HOOK_REGISTRATIONS
        .iter()
        .map(|r| r.trigger)
        .collect();
    expected_triggers.sort();
    expected_triggers.dedup();
    let mut actual_triggers: Vec<&str> = hooks.keys().map(String::as_str).collect();
    actual_triggers.sort();
    assert_eq!(actual_triggers, expected_triggers);
}

#[test]
fn snippet_claude_session_start_has_correct_shape() {
    let v = build_agent_snippet("claude", FAKE_HOOK).unwrap();
    let entries = v
        .pointer("/hooks/SessionStart")
        .and_then(Value::as_array)
        .expect("SessionStart should be an array");
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.get("matcher"), Some(&json!("")));
    let inner = entry
        .get("hooks")
        .and_then(Value::as_array)
        .expect("inner hooks array");
    assert_eq!(inner.len(), 1);
    assert_eq!(inner[0].get("type"), Some(&json!("command")));
    assert_eq!(
        inner[0].get("command"),
        Some(&json!("bash /fake/hook.sh claude session-start"))
    );
}

#[test]
fn snippet_claude_post_tool_use_maps_to_activity_log() {
    let v = build_agent_snippet("claude", FAKE_HOOK).unwrap();
    let cmd = v
        .pointer("/hooks/PostToolUse/0/hooks/0/command")
        .and_then(Value::as_str)
        .unwrap();
    assert_eq!(cmd, "bash /fake/hook.sh claude activity-log");
}

#[test]
fn snippet_codex_session_start_has_custom_matcher() {
    let v = build_agent_snippet("codex", FAKE_HOOK).unwrap();
    let entry = v
        .pointer("/hooks/SessionStart/0")
        .expect("codex SessionStart entry");
    assert_eq!(entry.get("matcher"), Some(&json!("startup|resume")));
    assert_eq!(
        entry
            .pointer("/hooks/0/command")
            .and_then(Value::as_str)
            .unwrap(),
        "bash /fake/hook.sh codex session-start"
    );
}

#[test]
fn snippet_codex_non_session_start_has_empty_matcher() {
    let v = build_agent_snippet("codex", FAKE_HOOK).unwrap();
    for reg in CodexAdapter::HOOK_REGISTRATIONS {
        if reg.trigger == "SessionStart" {
            continue;
        }
        let entry = v
            .pointer(&format!("/hooks/{}/0", reg.trigger))
            .unwrap_or_else(|| panic!("missing codex trigger {}", reg.trigger));
        assert_eq!(
            entry.get("matcher"),
            Some(&json!("")),
            "{} should have empty matcher",
            reg.trigger
        );
    }
}

#[test]
fn missing_hooks_is_empty_for_matching_claude_config() {
    let config = build_agent_snippet("claude", FAKE_HOOK).unwrap();
    assert!(missing_hooks("claude", &config, FAKE_HOOK).is_empty());
    assert!(!has_missing_hooks("claude", &config, FAKE_HOOK));
}

#[test]
fn missing_hooks_reports_removed_trigger() {
    let mut config = build_agent_snippet("claude", FAKE_HOOK).unwrap();
    let hooks = config
        .get_mut("hooks")
        .and_then(Value::as_object_mut)
        .expect("top-level hooks object");
    hooks.remove("SessionEnd");

    assert_eq!(
        missing_hooks("claude", &config, FAKE_HOOK),
        vec!["SessionEnd".to_string()]
    );
    assert!(has_missing_hooks("claude", &config, FAKE_HOOK));
}

#[test]
fn missing_hooks_reports_matcher_mismatch() {
    let mut config = build_agent_snippet("codex", FAKE_HOOK).unwrap();
    let hooks = config
        .get_mut("hooks")
        .and_then(Value::as_object_mut)
        .expect("top-level hooks object");
    let session_start = hooks
        .get_mut("SessionStart")
        .and_then(Value::as_array_mut)
        .expect("SessionStart array");
    let first = session_start[0]
        .as_object_mut()
        .expect("SessionStart entry object");

    first.insert("matcher".to_string(), json!(""));
    assert_eq!(
        missing_hooks("codex", &config, FAKE_HOOK),
        vec!["SessionStart".to_string()]
    );
}

#[test]
fn missing_hooks_reports_stale_command_path() {
    // A config that still lists the right trigger / matcher but points
    // at a non-existent hook.sh must be flagged — otherwise we would
    // silently lose hook delivery after a checkout move or rename.
    let mut config = build_agent_snippet("claude", FAKE_HOOK).unwrap();
    let hooks = config
        .get_mut("hooks")
        .and_then(Value::as_object_mut)
        .expect("top-level hooks object");
    let session_start = hooks
        .get_mut("SessionStart")
        .and_then(Value::as_array_mut)
        .expect("SessionStart array");
    let entry = session_start[0]
        .as_object_mut()
        .expect("SessionStart entry object");
    let actions = entry
        .get_mut("hooks")
        .and_then(Value::as_array_mut)
        .expect("inner hooks array");
    let command = actions[0].as_object_mut().expect("command hook object");
    command.insert(
        "command".to_string(),
        json!("bash /definitely/not/here/hook.sh claude session-start"),
    );

    assert_eq!(
        missing_hooks("claude", &config, FAKE_HOOK),
        vec!["SessionStart".to_string()]
    );
}

#[cfg(unix)]
#[test]
fn missing_hooks_accepts_symlinked_command_path() {
    // If the config command resolves (via canonicalize) to the same
    // real file as the expected command, it must be treated as a match
    // even if the literal strings differ. We use /tmp + a temp symlink
    // for a hermetic filesystem fixture.
    use std::io::Write;
    let tmp = std::env::temp_dir();
    let real_script = tmp.join(format!("mh-real-{}.sh", std::process::id()));
    {
        let mut f = std::fs::File::create(&real_script).unwrap();
        writeln!(f, "#!/bin/sh").unwrap();
    }
    let link_path = tmp.join(format!("mh-link-{}.sh", std::process::id()));
    let _ = std::fs::remove_file(&link_path);
    std::os::unix::fs::symlink(&real_script, &link_path).unwrap();

    let expected_hook = real_script.to_string_lossy().into_owned();
    let link_hook = link_path.to_string_lossy().into_owned();

    // Build the expected config against the real path, then swap the
    // command in the "current" config to the symlink path.
    let mut config = build_agent_snippet("claude", &expected_hook).unwrap();
    let hooks = config
        .get_mut("hooks")
        .and_then(Value::as_object_mut)
        .expect("top-level hooks object");
    let session_start = hooks
        .get_mut("SessionStart")
        .and_then(Value::as_array_mut)
        .expect("SessionStart array");
    let entry = session_start[0]
        .as_object_mut()
        .expect("SessionStart entry object");
    let actions = entry
        .get_mut("hooks")
        .and_then(Value::as_array_mut)
        .expect("inner hooks array");
    let command = actions[0].as_object_mut().expect("command hook object");
    command.insert(
        "command".to_string(),
        json!(format!("bash {} claude session-start", link_hook)),
    );

    // Both sides canonicalize to `real_script`, so SessionStart must
    // not appear in the missing list.
    let missing = missing_hooks("claude", &config, &expected_hook);
    assert!(
        !missing.contains(&"SessionStart".to_string()),
        "symlinked path should be treated as a match: missing = {:?}",
        missing
    );

    let _ = std::fs::remove_file(&link_path);
    let _ = std::fs::remove_file(&real_script);
}

#[cfg(unix)]
#[test]
fn missing_hooks_accepts_quoted_command_path_with_spaces() {
    // Regression: paths that need POSIX quoting (e.g. spaces) used to
    // break normalize_hook_command because it split on raw spaces and
    // treated `'/path` as the script. The expected and actual specs
    // diverged even though they pointed at the same real file.
    use std::io::Write;
    let tmp = std::env::temp_dir().join(format!("mh quoted {}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let real_script = tmp.join("hook.sh");
    {
        let mut f = std::fs::File::create(&real_script).unwrap();
        writeln!(f, "#!/bin/sh").unwrap();
    }
    let hook_path = real_script.to_string_lossy().into_owned();

    let config = build_agent_snippet("claude", &hook_path).unwrap();
    // Sanity check: the snippet really did emit a quoted command, so
    // we are exercising the parser, not just the unquoted fast path.
    let snippet_command = config
        .pointer("/hooks/SessionStart/0/hooks/0/command")
        .and_then(Value::as_str)
        .unwrap();
    assert!(
        snippet_command.contains("'"),
        "expected POSIX-quoted command for path with spaces, got {:?}",
        snippet_command
    );

    let missing = missing_hooks("claude", &config, &hook_path);
    assert!(
        missing.is_empty(),
        "quoted paths must round-trip through normalize_hook_command: missing = {:?}",
        missing
    );

    let _ = std::fs::remove_file(&real_script);
    let _ = std::fs::remove_dir(&tmp);
}

#[test]
fn missing_hooks_ignores_extra_entries() {
    let mut config = build_agent_snippet("claude", FAKE_HOOK).unwrap();
    let hooks = config
        .get_mut("hooks")
        .and_then(Value::as_object_mut)
        .expect("top-level hooks object");
    hooks.insert(
        "Bogus".to_string(),
        json!([
            {
                "matcher": "",
                "hooks": [
                    {
                        "type": "command",
                        "command": "bash /fake/hook.sh claude bogus"
                    }
                ]
            }
        ]),
    );

    assert!(missing_hooks("claude", &config, FAKE_HOOK).is_empty());
}

#[test]
fn missing_hooks_accepts_multiple_entries_and_actions_for_same_trigger() {
    let mut config = build_agent_snippet("claude", FAKE_HOOK).unwrap();
    let hooks = config
        .get_mut("hooks")
        .and_then(Value::as_object_mut)
        .expect("top-level hooks object");

    let session_start = hooks
        .get_mut("SessionStart")
        .and_then(Value::as_array_mut)
        .expect("SessionStart array");
    let mut duplicate_entry = session_start[0].clone();
    duplicate_entry
        .as_object_mut()
        .expect("SessionStart entry object")
        .get_mut("hooks")
        .and_then(Value::as_array_mut)
        .expect("inner hooks array")[0]
        .as_object_mut()
        .expect("command hook object")
        .insert(
            "command".to_string(),
            json!("bash /wrong/hook.sh claude session-start"),
        );
    session_start.push(duplicate_entry);

    let notification = hooks
        .get_mut("Notification")
        .and_then(Value::as_array_mut)
        .expect("Notification array");
    let notification_entry = notification[0]
        .as_object_mut()
        .expect("Notification entry object");
    let notification_actions = notification_entry
        .get_mut("hooks")
        .and_then(Value::as_array_mut)
        .expect("Notification hooks array");
    notification_actions.push(json!({
        "type": "command",
        "command": "bash /tmp/extra-notify.sh claude notification",
    }));

    assert!(missing_hooks("claude", &config, FAKE_HOOK).is_empty());
    assert!(!has_missing_hooks("claude", &config, FAKE_HOOK));
}

#[test]
fn full_output_has_expected_top_level_keys() {
    let v = build_setup_output(FAKE_HOOK);
    assert_eq!(
        v.get("version").and_then(Value::as_str),
        Some(crate::VERSION)
    );
    assert_eq!(
        v.get("hook_script").and_then(Value::as_str),
        Some(FAKE_HOOK)
    );
    let agents = v.get("agents").and_then(Value::as_object).unwrap();
    let mut keys: Vec<&str> = agents.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(keys, vec!["claude", "codex"]);
}

#[test]
fn full_output_snippet_matches_single_agent_snippet() {
    let full = build_setup_output(FAKE_HOOK);
    for agent in ["claude", "codex"] {
        let from_full = full
            .pointer(&format!("/agents/{}/snippet", agent))
            .unwrap_or_else(|| panic!("missing snippet for {}", agent));
        let from_single = build_agent_snippet(agent, FAKE_HOOK).unwrap();
        assert_eq!(from_full, &from_single, "drift for {}", agent);
    }
}

#[test]
fn full_output_normalized_hooks_count_matches_table() {
    let full = build_setup_output(FAKE_HOOK);
    for (agent, table_len) in [
        ("claude", ClaudeAdapter::HOOK_REGISTRATIONS.len()),
        ("codex", CodexAdapter::HOOK_REGISTRATIONS.len()),
    ] {
        let hooks = full
            .pointer(&format!("/agents/{}/hooks", agent))
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("missing hooks array for {}", agent));
        assert_eq!(
            hooks.len(),
            table_len,
            "normalized hooks[] length must match HOOK_REGISTRATIONS for {}",
            agent
        );
    }
}

#[test]
fn full_output_normalized_entry_shape() {
    let full = build_setup_output(FAKE_HOOK);
    let first = full.pointer("/agents/claude/hooks/0").unwrap();
    assert_eq!(first.get("trigger"), Some(&json!("SessionStart")));
    assert_eq!(first.get("matcher"), Some(&Value::Null));
    assert_eq!(first.get("event"), Some(&json!("session-start")));
    assert_eq!(
        first.get("command"),
        Some(&json!("bash /fake/hook.sh claude session-start"))
    );

    let codex_ss = full.pointer("/agents/codex/hooks/0").unwrap();
    assert_eq!(codex_ss.get("trigger"), Some(&json!("SessionStart")));
    assert_eq!(codex_ss.get("matcher"), Some(&json!("startup|resume")));
}

#[test]
fn full_output_config_paths() {
    let full = build_setup_output(FAKE_HOOK);
    assert_eq!(
        full.pointer("/agents/claude/config_path")
            .and_then(Value::as_str),
        Some("~/.claude/settings.json")
    );
    assert_eq!(
        full.pointer("/agents/codex/config_path")
            .and_then(Value::as_str),
        Some("~/.codex/hooks.json")
    );
}

#[test]
fn run_setup_no_args_returns_full_output() {
    let (code, json) = run_setup(&[], FAKE_HOOK);
    assert_eq!(code, 0);
    assert!(json.unwrap().get("agents").is_some());
}

#[test]
fn run_setup_claude_returns_only_snippet() {
    let (code, json) = run_setup(&["claude".to_string()], FAKE_HOOK);
    assert_eq!(code, 0);
    let v = json.unwrap();
    assert!(v.get("hooks").is_some());
    assert!(v.get("version").is_none());
    assert!(v.get("hook_script").is_none());
    assert!(v.get("agents").is_none());
}

#[test]
fn run_setup_codex_returns_only_snippet() {
    let (code, json) = run_setup(&["codex".to_string()], FAKE_HOOK);
    assert_eq!(code, 0);
    let v = json.unwrap();
    assert!(v.get("hooks").is_some());
    assert!(v.get("version").is_none());
}

#[test]
fn run_setup_unknown_agent_returns_err_exit_2() {
    let (code, json) = run_setup(&["gemini".to_string()], FAKE_HOOK);
    assert_eq!(code, 2);
    assert!(json.is_none());
}

#[test]
fn run_setup_too_many_args_returns_err_exit_2() {
    let (code, json) = run_setup(&["claude".to_string(), "extra".to_string()], FAKE_HOOK);
    assert_eq!(code, 2);
    assert!(json.is_none());
}

#[test]
fn full_output_snapshot() {
    let v = build_setup_output(FAKE_HOOK);
    let actual = serde_json::to_string_pretty(&v).unwrap();
    // Version-independent snapshot: substitute the placeholder at test
    // time so a version bump in Cargo.toml does not break this test.
    // When adapter tables legitimately change, temporarily add a
    // `println!` to inspect the new output and update the literal below.
    let expected = EXPECTED_FULL_OUTPUT.replace("__VERSION__", crate::VERSION);
    assert_eq!(
        actual, expected,
        "setup full output changed; update EXPECTED_FULL_OUTPUT in the \
             same commit that changes HOOK_REGISTRATIONS"
    );
}

const EXPECTED_FULL_OUTPUT: &str = r#"{
  "agents": {
    "claude": {
      "config_path": "~/.claude/settings.json",
      "hooks": [
        {
          "command": "bash /fake/hook.sh claude session-start",
          "event": "session-start",
          "matcher": null,
          "trigger": "SessionStart"
        },
        {
          "command": "bash /fake/hook.sh claude session-end",
          "event": "session-end",
          "matcher": null,
          "trigger": "SessionEnd"
        },
        {
          "command": "bash /fake/hook.sh claude user-prompt-submit",
          "event": "user-prompt-submit",
          "matcher": null,
          "trigger": "UserPromptSubmit"
        },
        {
          "command": "bash /fake/hook.sh claude notification",
          "event": "notification",
          "matcher": null,
          "trigger": "Notification"
        },
        {
          "command": "bash /fake/hook.sh claude stop",
          "event": "stop",
          "matcher": null,
          "trigger": "Stop"
        },
        {
          "command": "bash /fake/hook.sh claude stop-failure",
          "event": "stop-failure",
          "matcher": null,
          "trigger": "StopFailure"
        },
        {
          "command": "bash /fake/hook.sh claude permission-denied",
          "event": "permission-denied",
          "matcher": null,
          "trigger": "PermissionDenied"
        },
        {
          "command": "bash /fake/hook.sh claude cwd-changed",
          "event": "cwd-changed",
          "matcher": null,
          "trigger": "CwdChanged"
        },
        {
          "command": "bash /fake/hook.sh claude subagent-start",
          "event": "subagent-start",
          "matcher": null,
          "trigger": "SubagentStart"
        },
        {
          "command": "bash /fake/hook.sh claude subagent-stop",
          "event": "subagent-stop",
          "matcher": null,
          "trigger": "SubagentStop"
        },
        {
          "command": "bash /fake/hook.sh claude activity-log",
          "event": "activity-log",
          "matcher": null,
          "trigger": "PostToolUse"
        },
        {
          "command": "bash /fake/hook.sh claude task-created",
          "event": "task-created",
          "matcher": null,
          "trigger": "TaskCreated"
        },
        {
          "command": "bash /fake/hook.sh claude task-completed",
          "event": "task-completed",
          "matcher": null,
          "trigger": "TaskCompleted"
        },
        {
          "command": "bash /fake/hook.sh claude teammate-idle",
          "event": "teammate-idle",
          "matcher": null,
          "trigger": "TeammateIdle"
        },
        {
          "command": "bash /fake/hook.sh claude worktree-create",
          "event": "worktree-create",
          "matcher": null,
          "trigger": "WorktreeCreate"
        },
        {
          "command": "bash /fake/hook.sh claude worktree-remove",
          "event": "worktree-remove",
          "matcher": null,
          "trigger": "WorktreeRemove"
        }
      ],
      "snippet": {
        "hooks": {
          "CwdChanged": [
            {
              "hooks": [
                {
                  "command": "bash /fake/hook.sh claude cwd-changed",
                  "type": "command"
                }
              ],
              "matcher": ""
            }
          ],
          "Notification": [
            {
              "hooks": [
                {
                  "command": "bash /fake/hook.sh claude notification",
                  "type": "command"
                }
              ],
              "matcher": ""
            }
          ],
          "PermissionDenied": [
            {
              "hooks": [
                {
                  "command": "bash /fake/hook.sh claude permission-denied",
                  "type": "command"
                }
              ],
              "matcher": ""
            }
          ],
          "PostToolUse": [
            {
              "hooks": [
                {
                  "command": "bash /fake/hook.sh claude activity-log",
                  "type": "command"
                }
              ],
              "matcher": ""
            }
          ],
          "SessionEnd": [
            {
              "hooks": [
                {
                  "command": "bash /fake/hook.sh claude session-end",
                  "type": "command"
                }
              ],
              "matcher": ""
            }
          ],
          "SessionStart": [
            {
              "hooks": [
                {
                  "command": "bash /fake/hook.sh claude session-start",
                  "type": "command"
                }
              ],
              "matcher": ""
            }
          ],
          "Stop": [
            {
              "hooks": [
                {
                  "command": "bash /fake/hook.sh claude stop",
                  "type": "command"
                }
              ],
              "matcher": ""
            }
          ],
          "StopFailure": [
            {
              "hooks": [
                {
                  "command": "bash /fake/hook.sh claude stop-failure",
                  "type": "command"
                }
              ],
              "matcher": ""
            }
          ],
          "SubagentStart": [
            {
              "hooks": [
                {
                  "command": "bash /fake/hook.sh claude subagent-start",
                  "type": "command"
                }
              ],
              "matcher": ""
            }
          ],
          "SubagentStop": [
            {
              "hooks": [
                {
                  "command": "bash /fake/hook.sh claude subagent-stop",
                  "type": "command"
                }
              ],
              "matcher": ""
            }
          ],
          "TaskCompleted": [
            {
              "hooks": [
                {
                  "command": "bash /fake/hook.sh claude task-completed",
                  "type": "command"
                }
              ],
              "matcher": ""
            }
          ],
          "TaskCreated": [
            {
              "hooks": [
                {
                  "command": "bash /fake/hook.sh claude task-created",
                  "type": "command"
                }
              ],
              "matcher": ""
            }
          ],
          "TeammateIdle": [
            {
              "hooks": [
                {
                  "command": "bash /fake/hook.sh claude teammate-idle",
                  "type": "command"
                }
              ],
              "matcher": ""
            }
          ],
          "UserPromptSubmit": [
            {
              "hooks": [
                {
                  "command": "bash /fake/hook.sh claude user-prompt-submit",
                  "type": "command"
                }
              ],
              "matcher": ""
            }
          ],
          "WorktreeCreate": [
            {
              "hooks": [
                {
                  "command": "bash /fake/hook.sh claude worktree-create",
                  "type": "command"
                }
              ],
              "matcher": ""
            }
          ],
          "WorktreeRemove": [
            {
              "hooks": [
                {
                  "command": "bash /fake/hook.sh claude worktree-remove",
                  "type": "command"
                }
              ],
              "matcher": ""
            }
          ]
        }
      }
    },
    "codex": {
      "config_path": "~/.codex/hooks.json",
      "hooks": [
        {
          "command": "bash /fake/hook.sh codex session-start",
          "event": "session-start",
          "matcher": "startup|resume",
          "trigger": "SessionStart"
        },
        {
          "command": "bash /fake/hook.sh codex user-prompt-submit",
          "event": "user-prompt-submit",
          "matcher": null,
          "trigger": "UserPromptSubmit"
        },
        {
          "command": "bash /fake/hook.sh codex stop",
          "event": "stop",
          "matcher": null,
          "trigger": "Stop"
        },
        {
          "command": "bash /fake/hook.sh codex activity-log",
          "event": "activity-log",
          "matcher": null,
          "trigger": "PostToolUse"
        }
      ],
      "snippet": {
        "hooks": {
          "PostToolUse": [
            {
              "hooks": [
                {
                  "command": "bash /fake/hook.sh codex activity-log",
                  "type": "command"
                }
              ],
              "matcher": ""
            }
          ],
          "SessionStart": [
            {
              "hooks": [
                {
                  "command": "bash /fake/hook.sh codex session-start",
                  "type": "command"
                }
              ],
              "matcher": "startup|resume"
            }
          ],
          "Stop": [
            {
              "hooks": [
                {
                  "command": "bash /fake/hook.sh codex stop",
                  "type": "command"
                }
              ],
              "matcher": ""
            }
          ],
          "UserPromptSubmit": [
            {
              "hooks": [
                {
                  "command": "bash /fake/hook.sh codex user-prompt-submit",
                  "type": "command"
                }
              ],
              "matcher": ""
            }
          ]
        }
      }
    }
  },
  "hook_script": "/fake/hook.sh",
  "version": "__VERSION__"
}"#;

#[test]
fn full_output_normalized_command_matches_snippet_command() {
    let full = build_setup_output(FAKE_HOOK);
    for agent in ["claude", "codex"] {
        let hooks = full
            .pointer(&format!("/agents/{}/hooks", agent))
            .and_then(Value::as_array)
            .unwrap();
        for entry in hooks {
            let trigger = entry.get("trigger").and_then(Value::as_str).unwrap();
            let command = entry.get("command").and_then(Value::as_str).unwrap();
            let group = full
                .pointer(&format!("/agents/{}/snippet/hooks/{}", agent, trigger))
                .and_then(Value::as_array)
                .unwrap_or_else(|| panic!("snippet missing trigger {} for {}", trigger, agent));
            let found = group.iter().any(|slot: &Value| {
                slot.pointer("/hooks/0/command")
                    .and_then(Value::as_str)
                    .map(|c| c == command)
                    .unwrap_or(false)
            });
            assert!(
                found,
                "command {:?} missing from snippet of {}::{}",
                command, agent, trigger
            );
        }
    }
}
