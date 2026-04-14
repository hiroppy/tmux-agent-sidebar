# Setup CLI Subcommand Implementation Plan

> **Status: Completed.** Shipped under the name `setup` rather than `doctor`: module is `src/cli/setup.rs`, command is `tmux-agent-sidebar setup`, functions are `cmd_setup` / `build_setup_output` / `run_setup`. Every step below was executed — the `- [ ]` checkboxes are kept as historical record. Post-ship additions beyond this plan: POSIX shell quoting (`shell_quote` / `format_hook_command` in `setup.rs`) so install paths with spaces or metacharacters are safe, and a richer `ResolvedHookScript` return type from `resolve_hook_script`.

**Goal:** Add a `setup` CLI subcommand that prints the hooks Claude Code and Codex must register (and ready-to-paste config snippets) as JSON, derived entirely from the existing `HOOK_REGISTRATIONS` tables.

**Architecture:** One new file `src/cli/setup.rs` with three pure functions (`build_agent_snippet`, `build_setup_output`, `resolve_hook_script`) plus a thin `cmd_setup` CLI shell. Dispatch is wired into `src/cli/mod.rs`. The pure functions read only `ClaudeAdapter::HOOK_REGISTRATIONS` / `CodexAdapter::HOOK_REGISTRATIONS` and `AgentEventKind::external_name()` — no hook knowledge is duplicated. All tests run on the pure core with a fixed fake hook path.

**Tech Stack:** Rust 2024, `serde_json`, no new dependencies. Uses the existing `adapter::HookRegistration` struct and `event::AgentEventKind`.

**Spec:** _(never written; this plan was the sole design document)_

---

## File Structure

| File | Role |
|---|---|
| `src/cli/setup.rs` | **Create.** Owns `cmd_setup`, `build_setup_output`, `build_agent_snippet`, `resolve_hook_script`, and all unit tests. |
| `src/cli/mod.rs` | **Modify.** Add `mod setup;` and the `"setup"` match arm in `run()`. |
| `src/adapter/mod.rs` | **Read-only.** Source of `HookRegistration` type. Not edited. |
| `src/adapter/claude.rs` | **Read-only.** `ClaudeAdapter::HOOK_REGISTRATIONS` consumed. Not edited. |
| `src/adapter/codex.rs` | **Read-only.** `CodexAdapter::HOOK_REGISTRATIONS` consumed. Not edited. |
| `src/event.rs` | **Read-only.** `AgentEventKind::external_name()` consumed. Not edited. |

Nothing else is touched. README updates are out of scope for this plan.

---

## Task 1: Stub `setup` module and wire CLI dispatch

Gets the new module compiling and callable before adding any real logic, so later tasks can focus on behavior.

**Files:**
- Create: `src/cli/setup.rs`
- Modify: `src/cli/mod.rs` (top of file, and inside `run()`)

- [ ] **Step 1: Create stub module file**

Create `src/cli/setup.rs` with this exact content:

```rust
//! `setup` subcommand — prints required hooks and ready-to-paste config
//! snippets for Claude Code and Codex as JSON on stdout. Pure generator:
//! reads only the adapter `HOOK_REGISTRATIONS` tables, never the user's
//! config files.

pub(crate) fn cmd_setup(_args: &[String]) -> i32 {
    0
}
```

- [ ] **Step 2: Wire dispatch in `src/cli/mod.rs`**

At the top of `src/cli/mod.rs`, add `mod setup;` alongside the other module declarations. The existing block is:

```rust
mod hook;
mod label;
mod toggle;
```

Change it to:

```rust
mod setup;
mod hook;
mod label;
mod toggle;
```

Then in the `run()` function, add the `"setup"` arm to the match. The existing match contains:

```rust
    let code = match cmd {
        "hook" => hook::cmd_hook(rest),
        "toggle" => toggle::cmd_toggle(rest),
        "toggle-all" => toggle::cmd_toggle_all(rest),
        "auto-close" => toggle::cmd_auto_close(rest),
        "set-status" => cmd_set_status(rest),
        "--version" | "version" => {
            println!("{}", crate::VERSION);
            0
        }
        _ => return None,
    };
```

Change it to:

```rust
    let code = match cmd {
        "setup" => setup::cmd_setup(rest),
        "hook" => hook::cmd_hook(rest),
        "toggle" => toggle::cmd_toggle(rest),
        "toggle-all" => toggle::cmd_toggle_all(rest),
        "auto-close" => toggle::cmd_auto_close(rest),
        "set-status" => cmd_set_status(rest),
        "--version" | "version" => {
            println!("{}", crate::VERSION);
            0
        }
        _ => return None,
    };
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build`
Expected: successful build, no warnings about the stub (`_args` silences the unused-arg lint).

- [ ] **Step 4: Commit**

```bash
git add src/cli/setup.rs src/cli/mod.rs
git commit -m "feat(cli): scaffold setup subcommand"
```

---

## Task 2: Expose `HookRegistration` so `setup` can read it

The `HookRegistration` struct is currently `pub` but its containing module is `pub mod adapter` with the struct reachable as `crate::adapter::HookRegistration`. This task verifies reachability from `src/cli/setup.rs` with a compile-only test — no runtime behavior yet.

**Files:**
- Modify: `src/cli/setup.rs`

- [ ] **Step 1: Add a compile-only import**

Replace the content of `src/cli/setup.rs` with:

```rust
//! `setup` subcommand — prints required hooks and ready-to-paste config
//! snippets for Claude Code and Codex as JSON on stdout. Pure generator:
//! reads only the adapter `HOOK_REGISTRATIONS` tables, never the user's
//! config files.

use crate::adapter::HookRegistration;
use crate::adapter::claude::ClaudeAdapter;
use crate::adapter::codex::CodexAdapter;

#[allow(dead_code)]
const _CLAUDE_TABLE_REACHABLE: &[HookRegistration] = ClaudeAdapter::HOOK_REGISTRATIONS;
#[allow(dead_code)]
const _CODEX_TABLE_REACHABLE: &[HookRegistration] = CodexAdapter::HOOK_REGISTRATIONS;

pub(crate) fn cmd_setup(_args: &[String]) -> i32 {
    0
}
```

- [ ] **Step 2: Build to check visibility**

Run: `cargo build`
Expected: successful build. If it fails with a privacy error, `HookRegistration` / `ClaudeAdapter` / `CodexAdapter` / `HOOK_REGISTRATIONS` needs to be made `pub(crate)` or `pub` in the adapter module. Fix the visibility and rerun. Do **not** change any other adapter logic.

- [ ] **Step 3: Commit**

```bash
git add -u src/cli/setup.rs
git commit -m "chore(cli): pin doctor to adapter registration tables"
```

---

## Task 3: Implement `build_agent_snippet` (pure, TDD)

Builds the ready-to-paste `{ "hooks": { ... } }` JSON block for a single agent. This is the core of single-agent mode and is reused by full mode, so it must be correct before anything else.

**Files:**
- Modify: `src/cli/setup.rs`

- [ ] **Step 1: Write the failing tests**

Append a test module to `src/cli/setup.rs`. Add this after the existing code:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    const FAKE_HOOK: &str = "/fake/hook.sh";

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
        // One top-level key per unique trigger in the table.
        let mut expected_triggers: Vec<&str> = ClaudeAdapter::HOOK_REGISTRATIONS
            .iter()
            .map(|r| r.trigger)
            .collect();
        expected_triggers.sort();
        expected_triggers.dedup();
        let mut actual_triggers: Vec<&str> = hooks.keys().map(|s| s.as_str()).collect();
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
        // Known rename: upstream trigger `PostToolUse` → internal event `activity-log`.
        let v = build_agent_snippet("claude", FAKE_HOOK).unwrap();
        let cmd = v
            .pointer("/hooks/PostToolUse/0/hooks/0/command")
            .and_then(Value::as_str)
            .unwrap();
        assert_eq!(cmd, "bash /fake/hook.sh claude activity-log");
    }

    #[test]
    fn snippet_codex_session_start_has_custom_matcher() {
        // Codex SessionStart is the only registration with a non-None matcher.
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
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib cli::setup::tests`
Expected: all tests fail with `build_agent_snippet` not defined / not found.

- [ ] **Step 3: Implement `build_agent_snippet`**

Insert this function into `src/cli/setup.rs` just above the `cmd_setup` function (below the `_*_REACHABLE` consts):

```rust
/// Build the ready-to-paste `{ "hooks": { ... } }` JSON block for a single
/// agent. Returns `None` for unknown agent names.
///
/// Reads **only** from the adapter's `HOOK_REGISTRATIONS` table and
/// `AgentEventKind::external_name()` — no hook identity is duplicated here.
/// When `HookRegistration.matcher` is `None`, the snippet uses the empty
/// string `""` (matching Claude/Codex's "any tool" convention).
pub(crate) fn build_agent_snippet(
    agent: &str,
    hook_script: &str,
) -> Option<serde_json::Value> {
    let table: &[HookRegistration] = match agent {
        "claude" => ClaudeAdapter::HOOK_REGISTRATIONS,
        "codex" => CodexAdapter::HOOK_REGISTRATIONS,
        _ => return None,
    };

    let mut hooks = serde_json::Map::new();
    for reg in table {
        let matcher = reg.matcher.unwrap_or("");
        let command = format!(
            "bash {} {} {}",
            hook_script,
            agent,
            reg.kind.external_name()
        );
        let entry = serde_json::json!({
            "matcher": matcher,
            "hooks": [
                { "type": "command", "command": command }
            ],
        });
        // Group by trigger. Table order is preserved inside each group.
        let arr = hooks
            .entry(reg.trigger.to_string())
            .or_insert_with(|| serde_json::Value::Array(Vec::new()))
            .as_array_mut()
            .expect("trigger entry must be an array");
        arr.push(entry);
    }

    Some(serde_json::json!({ "hooks": serde_json::Value::Object(hooks) }))
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib cli::setup::tests`
Expected: all 7 tests pass.

- [ ] **Step 5: Run the whole suite to catch drift**

Run: `cargo test`
Expected: every test including `assert_table_drift_free` passes. This confirms `setup` is not reaching into adapter internals it shouldn't.

- [ ] **Step 6: Commit**

```bash
git add -u src/cli/setup.rs
git commit -m "feat(cli): add build_agent_snippet for setup command"
```

---

## Task 4: Implement `build_setup_output` (pure, TDD)

Wraps both agents' snippets plus normalized metadata into the full-mode output. Reuses `build_agent_snippet` so single- and full-mode views cannot drift.

**Files:**
- Modify: `src/cli/setup.rs`

- [ ] **Step 1: Write the failing tests**

Append these tests inside the existing `mod tests` block (after the Codex matcher test):

```rust
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
        let mut keys: Vec<&str> = agents.keys().map(|s| s.as_str()).collect();
        keys.sort();
        assert_eq!(keys, vec!["claude", "codex"]);
    }

    #[test]
    fn full_output_snippet_matches_single_agent_snippet() {
        // The two views of the same data MUST be byte-identical.
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
        // Spot-check a Claude entry.
        let first = full.pointer("/agents/claude/hooks/0").unwrap();
        assert_eq!(first.get("trigger"), Some(&json!("SessionStart")));
        assert_eq!(first.get("matcher"), Some(&Value::Null));
        assert_eq!(first.get("event"), Some(&json!("session-start")));
        assert_eq!(
            first.get("command"),
            Some(&json!("bash /fake/hook.sh claude session-start"))
        );

        // And a Codex entry where matcher is Some(...).
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
    fn full_output_normalized_command_matches_snippet_command() {
        // For every registration, the normalized `command` and the snippet's
        // inner `command` must be byte-identical. Prevents the two views
        // from drifting.
        let full = build_setup_output(FAKE_HOOK);
        for agent in ["claude", "codex"] {
            let hooks = full
                .pointer(&format!("/agents/{}/hooks", agent))
                .and_then(Value::as_array)
                .unwrap();
            for entry in hooks {
                let trigger = entry.get("trigger").and_then(Value::as_str).unwrap();
                let command = entry.get("command").and_then(Value::as_str).unwrap();
                // Walk the snippet for the same trigger and look for a matching command.
                let group = full
                    .pointer(&format!("/agents/{}/snippet/hooks/{}", agent, trigger))
                    .and_then(Value::as_array)
                    .unwrap_or_else(|| panic!("snippet missing trigger {} for {}", trigger, agent));
                let found = group.iter().any(|slot| {
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib cli::setup::tests`
Expected: the new tests fail with `build_setup_output` not defined.

- [ ] **Step 3: Implement `build_setup_output`**

Insert this function into `src/cli/setup.rs` just below `build_agent_snippet`:

```rust
/// Build the full setup output: version, resolved hook script path,
/// and a per-agent object containing `config_path`, the normalized
/// `hooks[]` array, and the ready-to-paste `snippet`.
///
/// This is a pure function. `hook_script` is passed in so tests can pin it
/// to a fixed string.
pub(crate) fn build_setup_output(hook_script: &str) -> serde_json::Value {
    let claude = build_agent_entry(
        "claude",
        "~/.claude/settings.json",
        ClaudeAdapter::HOOK_REGISTRATIONS,
        hook_script,
    );
    let codex = build_agent_entry(
        "codex",
        "~/.codex/hooks.json",
        CodexAdapter::HOOK_REGISTRATIONS,
        hook_script,
    );

    serde_json::json!({
        "version": crate::VERSION,
        "hook_script": hook_script,
        "agents": {
            "claude": claude,
            "codex": codex,
        },
    })
}

fn build_agent_entry(
    agent: &str,
    config_path: &str,
    table: &[HookRegistration],
    hook_script: &str,
) -> serde_json::Value {
    let hooks: Vec<serde_json::Value> = table
        .iter()
        .map(|reg| {
            let command = format!(
                "bash {} {} {}",
                hook_script,
                agent,
                reg.kind.external_name()
            );
            serde_json::json!({
                "trigger": reg.trigger,
                "matcher": match reg.matcher {
                    Some(m) => serde_json::Value::String(m.to_string()),
                    None => serde_json::Value::Null,
                },
                "event": reg.kind.external_name(),
                "command": command,
            })
        })
        .collect();

    let snippet = build_agent_snippet(agent, hook_script)
        .expect("agent name hardcoded above, must match build_agent_snippet");

    serde_json::json!({
        "config_path": config_path,
        "hooks": hooks,
        "snippet": snippet,
    })
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib cli::setup::tests`
Expected: all 13 tests in the module pass (7 from Task 3 + 6 new).

- [ ] **Step 5: Run the whole suite**

Run: `cargo test`
Expected: full green.

- [ ] **Step 6: Commit**

```bash
git add -u src/cli/setup.rs
git commit -m "feat(cli): add build_setup_output for doctor full mode"
```

---

## Task 5: Implement `resolve_hook_script` helper

Resolves the absolute path of `hook.sh` from the running binary location, with a README-matching fallback. Not unit-tested (depends on `current_exe()` and the filesystem), but kept small and obvious.

**Files:**
- Modify: `src/cli/setup.rs`

- [ ] **Step 1: Add the helper**

Insert this function into `src/cli/setup.rs` above `cmd_setup`:

```rust
/// Resolve the absolute path of `hook.sh` to embed in the generated
/// commands. Strategy:
///
/// 1. `std::env::current_exe()` → get the running binary path.
/// 2. Walk up at most 3 directories from its parent, checking for a
///    sibling `hook.sh` at each level. Matches the two layouts the
///    project already supports:
///      - `<plugin>/bin/tmux-agent-sidebar` → `<plugin>/hook.sh`
///      - `<plugin>/target/release/tmux-agent-sidebar` → `<plugin>/hook.sh`
/// 3. Fallback: the literal string `~/.tmux/plugins/tmux-agent-sidebar/hook.sh`
///    (tilde intentionally not expanded, matches README).
///
/// Never panics. Returns `String` directly, not `Result`.
fn resolve_hook_script() -> String {
    const FALLBACK: &str = "~/.tmux/plugins/tmux-agent-sidebar/hook.sh";

    let Ok(exe) = std::env::current_exe() else {
        return FALLBACK.to_string();
    };
    let Some(mut dir) = exe.parent().map(|p| p.to_path_buf()) else {
        return FALLBACK.to_string();
    };
    for _ in 0..=3 {
        let candidate = dir.join("hook.sh");
        if candidate.is_file() {
            return candidate.to_string_lossy().into_owned();
        }
        match dir.parent() {
            Some(parent) => dir = parent.to_path_buf(),
            None => break,
        }
    }
    FALLBACK.to_string()
}
```

- [ ] **Step 2: Build to verify it compiles**

Run: `cargo build`
Expected: successful build. The function is unused so far — expect a dead-code warning, which Step 1 of Task 6 will resolve.

- [ ] **Step 3: Commit**

```bash
git add -u src/cli/setup.rs
git commit -m "feat(cli): add hook.sh path resolver for doctor"
```

---

## Task 6: Wire `cmd_setup` dispatch and argument parsing

Turn the stub `cmd_setup` into the real CLI shell: resolve the hook path, dispatch on argument count, print pretty JSON, handle errors.

**Files:**
- Modify: `src/cli/setup.rs`

- [ ] **Step 1: Write the failing dispatch tests**

Append to the `mod tests` block:

```rust
    // Helper: invoke cmd_setup with captured stdout is not straightforward
    // without a harness, so these tests exercise the pure dispatch core
    // (`run_setup`) that cmd_setup will delegate to.

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
        // Snippet shape only: top-level must be exactly { "hooks": {...} }.
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
        let (code, json) = run_setup(
            &["claude".to_string(), "extra".to_string()],
            FAKE_HOOK,
        );
        assert_eq!(code, 2);
        assert!(json.is_none());
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib cli::setup::tests`
Expected: the new tests fail with `run_setup` not found.

- [ ] **Step 3: Implement `run_setup` and rewrite `cmd_setup`**

Replace the existing stub:

```rust
pub(crate) fn cmd_setup(_args: &[String]) -> i32 {
    0
}
```

with:

```rust
/// Pure dispatch core for `cmd_setup`. Returns the exit code and the JSON
/// value to print (or `None` if nothing should be printed, e.g. on error).
/// Splitting this out keeps `cmd_setup` a pure I/O wrapper.
fn run_setup(args: &[String], hook_script: &str) -> (i32, Option<serde_json::Value>) {
    match args.len() {
        0 => (0, Some(build_setup_output(hook_script))),
        1 => match build_agent_snippet(&args[0], hook_script) {
            Some(snippet) => (0, Some(snippet)),
            None => {
                eprintln!(
                    "error: unknown agent '{}' (expected 'claude' or 'codex')",
                    args[0]
                );
                (2, None)
            }
        },
        _ => {
            eprintln!("usage: tmux-agent-sidebar setup [claude|codex]");
            (2, None)
        }
    }
}

pub(crate) fn cmd_setup(args: &[String]) -> i32 {
    let hook_script = resolve_hook_script();
    let (code, json) = run_setup(args, &hook_script);
    if let Some(v) = json {
        match serde_json::to_string_pretty(&v) {
            Ok(s) => println!("{}", s),
            Err(e) => {
                eprintln!("error: failed to serialize setup output: {}", e);
                return 1;
            }
        }
    }
    code
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib cli::setup::tests`
Expected: all 18 tests pass (13 from earlier + 5 new dispatch tests).

- [ ] **Step 5: Run the full suite**

Run: `cargo test`
Expected: full green.

- [ ] **Step 6: Commit**

```bash
git add -u src/cli/setup.rs
git commit -m "feat(cli): wire setup command dispatch and output"
```

---

## Task 7: Snapshot test for the full output

Lock the full JSON shape with a single snapshot. Any accidental schema change — key rename, reordering, etc. — trips this test and forces explicit acknowledgement in the PR.

**Files:**
- Modify: `src/cli/setup.rs`

- [ ] **Step 1: Generate the current snapshot**

Temporarily add this scratch test to `mod tests` to dump the current output:

```rust
    #[test]
    #[ignore]
    fn dump_snapshot() {
        let v = build_setup_output(FAKE_HOOK);
        println!("{}", serde_json::to_string_pretty(&v).unwrap());
        panic!("dump");
    }
```

Run: `cargo test --lib cli::setup::tests::dump_snapshot -- --ignored --nocapture`
Expected: the test panics and prints the full JSON to stdout. Copy the printed JSON exactly.

- [ ] **Step 2: Replace the scratch test with a real snapshot test**

Remove `dump_snapshot` and add:

```rust
    #[test]
    fn full_output_snapshot() {
        let v = build_setup_output(FAKE_HOOK);
        let actual = serde_json::to_string_pretty(&v).unwrap();
        // Paste the exact dump from Step 1 between the raw string delimiters.
        // When adapter tables legitimately change, regenerate this by running
        // the `dump_snapshot` test and updating the literal.
        let expected = r#"<<< PASTE FROM STEP 1 >>>"#;
        assert_eq!(
            actual, expected,
            "setup full output changed; regenerate the snapshot via the \
             temporary dump_snapshot test and update this literal in the \
             same commit"
        );
    }
```

Replace `<<< PASTE FROM STEP 1 >>>` with the literal JSON captured in Step 1. The `r#"..."#` raw string lets you keep the JSON's double-quotes without escaping. The version field comes from `crate::VERSION`, which is `env!("CARGO_PKG_VERSION")` at compile time — the snapshot is deterministic across any checkout of this commit.

- [ ] **Step 3: Run the test**

Run: `cargo test --lib cli::setup::tests::full_output_snapshot`
Expected: PASS.

- [ ] **Step 4: Run the full suite**

Run: `cargo test`
Expected: full green.

- [ ] **Step 5: Commit**

```bash
git add -u src/cli/setup.rs
git commit -m "test(cli): snapshot doctor full output"
```

---

## Task 8: Final integration checks

Verify the command works end-to-end outside of tests, including clippy, formatting, and a manual smoke test.

**Files:** none

- [ ] **Step 1: Run clippy**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: no warnings. If clippy flags something, fix it in `src/cli/setup.rs` and re-run.

- [ ] **Step 2: Run formatter**

Run: `cargo fmt`
Expected: no diff. If there is a diff, commit it separately with:

```bash
git add -u
git commit -m "style: cargo fmt doctor module"
```

- [ ] **Step 3: Build release and smoke-test**

Run:
```bash
cargo build --release
./target/release/tmux-agent-sidebar setup | head -20
```
Expected: pretty-printed JSON starting with `{`, containing `"version"`, `"hook_script"`, and `"agents"` keys. The `hook_script` value should either be an absolute path ending in `hook.sh` (if run inside the repo checkout) or the `~/.tmux/plugins/...` fallback.

Then:
```bash
./target/release/tmux-agent-sidebar setup claude | head -20
```
Expected: JSON starting with `{ "hooks": {`. No `version` or `hook_script` keys at top level.

```bash
./target/release/tmux-agent-sidebar setup codex
```
Expected: same shape as above, Codex hooks, `SessionStart` entry has `"matcher": "startup|resume"`.

```bash
./target/release/tmux-agent-sidebar setup gemini; echo "exit=$?"
```
Expected: stderr shows `error: unknown agent 'gemini' (expected 'claude' or 'codex')`, exit code `2`.

- [ ] **Step 4: Confirm the full-output snippet is still valid JSON**

Run:
```bash
./target/release/tmux-agent-sidebar setup | python3 -m json.tool >/dev/null && echo ok
./target/release/tmux-agent-sidebar setup claude | python3 -m json.tool >/dev/null && echo ok
./target/release/tmux-agent-sidebar setup codex | python3 -m json.tool >/dev/null && echo ok
```
Expected: three `ok` lines. If `python3` is not available, substitute `jq .` or `node -e "JSON.parse(require('fs').readFileSync(0,'utf8'))"`.

- [ ] **Step 5: Nothing to commit**

If everything passed, there is nothing to commit in this task. If clippy or fmt made changes, they were already committed in their own steps.
