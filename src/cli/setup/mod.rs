//! `setup` subcommand — prints required hooks and ready-to-paste config
//! snippets for Claude Code, Codex, and Cursor as JSON on stdout. Pure
//! generator: reads only the adapter `HOOK_REGISTRATIONS` tables, never the
//! user's config files.

use std::path::PathBuf;

use crate::adapter::HookRegistration;
use crate::adapter::claude::ClaudeAdapter;
use crate::adapter::codex::CodexAdapter;
use crate::adapter::cursor::CursorAdapter;

#[allow(dead_code)]
const _CLAUDE_TABLE_REACHABLE: &[HookRegistration] = ClaudeAdapter::HOOK_REGISTRATIONS;
#[allow(dead_code)]
const _CODEX_TABLE_REACHABLE: &[HookRegistration] = CodexAdapter::HOOK_REGISTRATIONS;
#[allow(dead_code)]
const _CURSOR_TABLE_REACHABLE: &[HookRegistration] = CursorAdapter::HOOK_REGISTRATIONS;

/// Shape of the agent's hook config file. Claude Code and Codex share one
/// schema; Cursor uses a flatter one, so the snippet generator and the
/// `missing_hooks` comparator both have to branch on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HookConfigStyle {
    /// `{"hooks": {Trigger: [{"matcher": …, "hooks": [{"type": "command",
    /// "command": …}]}]}}` — Claude Code and Codex.
    Nested,
    /// `{"version": 1, "hooks": {trigger: [{"type": "command", "command":
    /// …}]}}` — Cursor. Commands sit directly in the trigger array with no
    /// matcher wrapper, and the file carries a schema `version`.
    Flat,
}

/// Per-agent wiring the generator needs: which registration table to read,
/// where the config file lives, and which schema to emit.
struct AgentSpec {
    config_path: &'static str,
    table: &'static [HookRegistration],
    style: HookConfigStyle,
}

fn agent_spec(agent: &str) -> Option<AgentSpec> {
    match agent {
        "claude" => Some(AgentSpec {
            config_path: "~/.claude/settings.json",
            table: ClaudeAdapter::HOOK_REGISTRATIONS,
            style: HookConfigStyle::Nested,
        }),
        "codex" => Some(AgentSpec {
            config_path: "~/.codex/hooks.json",
            table: CodexAdapter::HOOK_REGISTRATIONS,
            style: HookConfigStyle::Nested,
        }),
        "cursor" => Some(AgentSpec {
            config_path: "~/.cursor/hooks.json",
            table: CursorAdapter::HOOK_REGISTRATIONS,
            style: HookConfigStyle::Flat,
        }),
        _ => None,
    }
}

/// Agents the setup generator knows how to wire, in the order they appear in
/// `setup` output. OpenCode is intentionally absent — it uses the bundled JS
/// plugin bridge instead of a hook config file.
const SETUP_AGENTS: &[&str] = &["claude", "codex", "cursor"];

/// POSIX-quote a string for safe use as a single shell argument.
///
/// Fast path: when the string consists only of characters that bash does
/// not interpret specially, it is returned as-is. This matters for the
/// common case (`/Users/alice/.../hook.sh`) because aggressive quoting
/// would suppress tilde expansion on the fallback path
/// `~/.tmux/plugins/tmux-agent-sidebar/hook.sh` and break the emitted
/// hook commands. This mirrors Python's `shlex.quote` behaviour.
///
/// Slow path: wrap the value in single quotes and escape any internal
/// single quotes as `'\''`. Safe for paths containing spaces, `$`, `;`,
/// backticks, and other shell metacharacters.
pub(crate) fn shell_quote(s: &str) -> String {
    fn is_safe(c: char) -> bool {
        c.is_ascii_alphanumeric()
            || matches!(c, '/' | '-' | '_' | '.' | '~' | '+' | '=' | ',' | '@' | ':')
    }
    if !s.is_empty() && s.chars().all(is_safe) {
        return s.to_string();
    }

    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Build the `bash <hook_script> <agent> <event>` command string with
/// proper POSIX quoting so arbitrary installation paths are safe.
fn format_hook_command(hook_script: &str, agent: &str, event: &str) -> String {
    format!("bash {} {} {}", shell_quote(hook_script), agent, event)
}

/// Build the ready-to-paste `{ "hooks": { ... } }` JSON block for a single
/// agent. Returns `None` for unknown agent names.
///
/// Reads **only** from the adapter's `HOOK_REGISTRATIONS` table and
/// `AgentEventKind::external_name()` — no hook identity is duplicated here.
/// When `HookRegistration.matcher` is `None`, the nested (Claude/Codex)
/// schema uses the empty string `""` — their "any tool" convention — while
/// the flat (Cursor) schema omits the key entirely, since Cursor treats an
/// absent matcher as "match everything" and only accepts one on a subset of
/// its triggers.
pub(crate) fn build_agent_snippet(agent: &str, hook_script: &str) -> Option<serde_json::Value> {
    let spec = agent_spec(agent)?;

    let mut hooks = serde_json::Map::new();
    for reg in spec.table {
        let command = format_hook_command(hook_script, agent, reg.kind.external_name());
        let entry = match spec.style {
            HookConfigStyle::Nested => serde_json::json!({
                "matcher": reg.matcher.unwrap_or(""),
                "hooks": [
                    { "type": "command", "command": command }
                ],
            }),
            HookConfigStyle::Flat => {
                let mut entry = serde_json::Map::new();
                entry.insert("type".into(), serde_json::json!("command"));
                entry.insert("command".into(), serde_json::json!(command));
                if let Some(matcher) = reg.matcher {
                    entry.insert("matcher".into(), serde_json::json!(matcher));
                }
                serde_json::Value::Object(entry)
            }
        };
        let arr = hooks
            .entry(reg.trigger.to_string())
            .or_insert_with(|| serde_json::Value::Array(Vec::new()))
            .as_array_mut()
            .expect("trigger entry must be an array");
        arr.push(entry);
    }

    let hooks = serde_json::Value::Object(hooks);
    Some(match spec.style {
        HookConfigStyle::Nested => serde_json::json!({ "hooks": hooks }),
        // Cursor rejects a hooks.json without a schema `version`.
        HookConfigStyle::Flat => serde_json::json!({ "version": 1, "hooks": hooks }),
    })
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct HookSpec {
    trigger: String,
    matcher: String,
    command: String,
}

#[allow(dead_code)]
fn normalize_matcher(value: Option<&serde_json::Value>) -> String {
    value.and_then(|v| v.as_str()).unwrap_or("").to_string()
}

#[allow(dead_code)]
fn collect_hook_specs(config: &serde_json::Value, style: HookConfigStyle) -> Vec<HookSpec> {
    let Some(hooks) = config.get("hooks").and_then(serde_json::Value::as_object) else {
        return Vec::new();
    };

    let mut specs = Vec::new();
    for (trigger, entries) in hooks {
        let Some(entries) = entries.as_array() else {
            continue;
        };
        for entry in entries {
            let matcher = normalize_matcher(entry.get("matcher"));
            match style {
                HookConfigStyle::Nested => {
                    let Some(actions) = entry.get("hooks").and_then(serde_json::Value::as_array)
                    else {
                        continue;
                    };
                    for action in actions {
                        if let Some(command) = command_of(action, false) {
                            specs.push(HookSpec {
                                trigger: trigger.clone(),
                                matcher: matcher.clone(),
                                command,
                            });
                        }
                    }
                }
                // Flat entries *are* the action, so there is no inner array
                // to walk.
                HookConfigStyle::Flat => {
                    if let Some(command) = command_of(entry, true) {
                        specs.push(HookSpec {
                            trigger: trigger.clone(),
                            matcher,
                            command,
                        });
                    }
                }
            }
        }
    }

    specs
}

/// Extract the `command` from a hook action, skipping non-command actions.
/// `type_optional` is `true` only for Cursor, whose schema documents `type`
/// as defaulting to `"command"`; Claude and Codex always spell it out, so a
/// missing key there means the entry is not one of ours.
#[allow(dead_code)]
fn command_of(action: &serde_json::Value, type_optional: bool) -> Option<String> {
    match action.get("type").and_then(serde_json::Value::as_str) {
        Some("command") => {}
        None if type_optional => {}
        _ => return None,
    }
    Some(
        action
            .get("command")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
    )
}

/// Return the trigger names that are required by `agent` but missing from
/// `current_config`.
///
/// Comparison uses `trigger`, `matcher`, and the canonicalized hook command.
/// The command path is resolved through `std::fs::canonicalize` so a
/// symlinked plugin directory (`~/.tmux/plugins/tmux-agent-sidebar/hook.sh`
/// → `~/Programming/tmux-agent-sidebar/hook.sh`) still compares equal, while
/// configs pointing at a stale or renamed checkout canonicalize to a
/// different real path (or fail to canonicalize at all) and are flagged as
/// missing.
#[allow(dead_code)]
pub(crate) fn missing_hooks(
    agent: &str,
    current_config: &serde_json::Value,
    hook_script: &str,
) -> Vec<String> {
    let Some(spec) = agent_spec(agent) else {
        return Vec::new();
    };
    let Some(expected) = build_agent_snippet(agent, hook_script) else {
        return Vec::new();
    };

    let expected = collect_hook_specs(&expected, spec.style);
    let actual: std::collections::HashSet<HookSpec> =
        collect_hook_specs(current_config, spec.style)
            .into_iter()
            .map(normalize_hook_spec)
            .collect();

    let mut missing = Vec::new();
    let mut seen_triggers = std::collections::BTreeSet::new();
    for spec in expected {
        let key = normalize_hook_spec(spec.clone());
        if actual.contains(&key) || !seen_triggers.insert(spec.trigger.clone()) {
            continue;
        }
        missing.push(spec.trigger);
    }
    missing
}

/// Normalize a hook spec so path drift that still points at the same file
/// (e.g. via a plugin-dir symlink) compares equal, while a broken or stale
/// path stays distinct.
fn normalize_hook_spec(mut spec: HookSpec) -> HookSpec {
    spec.command = normalize_hook_command(&spec.command);
    spec
}

/// Canonicalize the script path inside a `bash <path> <args...>` command.
/// Paths that fail to canonicalize (missing file, unresolved `~`) are
/// returned with tilde expansion only, so a stale config does not collapse
/// onto the expected command by accident.
///
/// `format_hook_command` POSIX-quotes paths that contain spaces or other
/// shell metacharacters (e.g. `bash '/path with spaces/hook.sh' …`), so
/// the parser must understand single-quoted scripts — splitting on raw
/// spaces would treat `'/path` as the script and break canonicalization
/// for exactly the installs that need quoting.
fn normalize_hook_command(cmd: &str) -> String {
    let Some((head, rest)) = cmd.split_once(' ') else {
        return cmd.to_string();
    };
    let rest = rest.trim_start_matches(' ');

    let (script, tail) = if let Some(after) = rest.strip_prefix('\'') {
        // POSIX single-quoted: the next `'` ends the script. We do not
        // try to honour the `'\''` escape sequence — paths containing a
        // literal single quote are vanishingly rare and not worth the
        // complexity here.
        match after.find('\'') {
            Some(end) => (
                after[..end].to_string(),
                after[end + 1..].trim_start_matches(' ').to_string(),
            ),
            None => return cmd.to_string(),
        }
    } else if let Some(after) = rest.strip_prefix('"') {
        match after.find('"') {
            Some(end) => (
                after[..end].to_string(),
                after[end + 1..].trim_start_matches(' ').to_string(),
            ),
            None => return cmd.to_string(),
        }
    } else {
        match rest.split_once(' ') {
            Some((s, t)) => (s.to_string(), t.to_string()),
            None => (rest.to_string(), String::new()),
        }
    };

    let expanded = expand_home_tilde(&script);
    let resolved = std::fs::canonicalize(&expanded)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or(expanded);
    // Re-quote with the same rules `format_hook_command` uses so the
    // expected and actual specs compare equal byte-for-byte.
    let quoted = shell_quote(&resolved);
    if tail.is_empty() {
        format!("{} {}", head, quoted)
    } else {
        format!("{} {} {}", head, quoted, tail)
    }
}

/// Minimal `~/`-to-`$HOME` expansion so we can normalize config commands
/// without pulling in a shellexpand dep.
fn expand_home_tilde(path: &str) -> String {
    let Some(rest) = path.strip_prefix("~/") else {
        return path.to_string();
    };
    let Some(home) = std::env::var_os("HOME") else {
        return path.to_string();
    };
    let mut p = std::path::PathBuf::from(home);
    p.push(rest);
    p.to_string_lossy().into_owned()
}

#[allow(dead_code)]
pub(crate) fn has_missing_hooks(
    agent: &str,
    current_config: &serde_json::Value,
    hook_script: &str,
) -> bool {
    !missing_hooks(agent, current_config, hook_script).is_empty()
}

/// Build the full setup output: version, resolved hook script path,
/// and a per-agent object containing `config_path`, the normalized
/// `hooks[]` array, and the ready-to-paste `snippet`.
///
/// Pure function. `hook_script` is passed in so tests can pin it.
pub(crate) fn build_setup_output(hook_script: &str) -> serde_json::Value {
    let agents: serde_json::Map<String, serde_json::Value> = SETUP_AGENTS
        .iter()
        .map(|agent| ((*agent).to_string(), build_agent_entry(agent, hook_script)))
        .collect();

    serde_json::json!({
        "version": crate::VERSION,
        "hook_script": hook_script,
        "agents": serde_json::Value::Object(agents),
    })
}

fn build_agent_entry(agent: &str, hook_script: &str) -> serde_json::Value {
    let spec = agent_spec(agent).expect("SETUP_AGENTS entries must all resolve through agent_spec");
    let hooks: Vec<serde_json::Value> = spec
        .table
        .iter()
        .map(|reg| {
            let command = format_hook_command(hook_script, agent, reg.kind.external_name());
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
        .expect("agent_spec resolved above, so build_agent_snippet must too");

    serde_json::json!({
        "config_path": spec.config_path,
        "hooks": hooks,
        "snippet": snippet,
    })
}

/// Result of attempting to locate `hook.sh` relative to the running binary.
/// The `detected` flag is `false` when the resolver could not find an
/// actual file on disk and had to return the README fallback — callers
/// should warn the user in that case because the emitted commands will be
/// wrong for non-default installs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedHookScript {
    pub path: String,
    pub detected: bool,
}

const FALLBACK_HOOK_SCRIPT: &str = "~/.tmux/plugins/tmux-agent-sidebar/hook.sh";

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
/// When step 1 or 2 succeeds, `detected = true`. When step 3 kicks in,
/// `detected = false` and `cmd_setup` surfaces a stderr warning. Never
/// panics.
pub(crate) fn resolve_hook_script() -> ResolvedHookScript {
    walk_up_from_exe(3, |dir| {
        let candidate = dir.join("hook.sh");
        candidate
            .is_file()
            .then(|| candidate.to_string_lossy().into_owned())
    })
    .map(|path| ResolvedHookScript {
        path,
        detected: true,
    })
    .unwrap_or_else(|| ResolvedHookScript {
        path: FALLBACK_HOOK_SCRIPT.to_string(),
        detected: false,
    })
}

/// Walk up from the running binary (`current_exe()`) and invoke `probe`
/// at each ancestor directory, stopping at the first non-`None` result
/// or after `max_depth` steps. Returns `None` when the executable path
/// cannot be resolved (e.g. exotic platforms) — callers must be prepared
/// to fall back gracefully. Used by `resolve_hook_script` and by
/// `notices::plugin_root_from_exe` so the upward-walk loop lives in one
/// place.
pub(crate) fn walk_up_from_exe<F, T>(max_depth: usize, mut probe: F) -> Option<T>
where
    F: FnMut(&std::path::Path) -> Option<T>,
{
    let exe = std::env::current_exe().ok()?;
    let mut dir = exe.parent()?.to_path_buf();
    for _ in 0..=max_depth {
        if let Some(value) = probe(&dir) {
            return Some(value);
        }
        dir = dir.parent()?.to_path_buf();
    }
    None
}

/// Return the default config path for `agent` under the current user's home.
#[allow(dead_code)]
pub(crate) fn config_path_for_agent(agent: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let relative = agent_spec(agent)?.config_path.strip_prefix("~/")?;
    Some(PathBuf::from(home).join(relative))
}

/// Load the current config for `agent`.
///
/// Missing files, unreadable files, or invalid JSON all map to `Value::Null`
/// so callers can still use `missing_hooks()` and surface the full expected
/// hook set.
#[allow(dead_code)]
pub(crate) fn load_current_config(agent: &str) -> serde_json::Value {
    let Some(path) = config_path_for_agent(agent) else {
        return serde_json::Value::Null;
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return serde_json::Value::Null;
    };
    serde_json::from_str(&text).unwrap_or(serde_json::Value::Null)
}

/// Pure dispatch core. Returns the exit code and the JSON to print
/// (or `None` if nothing should be printed, e.g. on error). Splitting
/// this out keeps `cmd_setup` a thin I/O wrapper.
fn run_setup(args: &[String], hook_script: &str) -> (i32, Option<serde_json::Value>) {
    match args.len() {
        0 => (0, Some(build_setup_output(hook_script))),
        1 => match build_agent_snippet(&args[0], hook_script) {
            Some(snippet) => (0, Some(snippet)),
            None => {
                eprintln!(
                    "error: unknown agent '{}' (expected one of: {})",
                    args[0],
                    SETUP_AGENTS.join(", ")
                );
                (2, None)
            }
        },
        _ => {
            eprintln!(
                "usage: tmux-agent-sidebar setup [{}]",
                SETUP_AGENTS.join("|")
            );
            (2, None)
        }
    }
}

pub(crate) fn cmd_setup(args: &[String]) -> i32 {
    let resolved = resolve_hook_script();
    if !resolved.detected {
        eprintln!(
            "warning: could not locate hook.sh relative to the running \
             binary; using fallback path {:?}. If your installation lives \
             elsewhere, hand-edit the 'command' values before pasting.",
            resolved.path
        );
    }
    let (code, json) = run_setup(args, &resolved.path);
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

#[cfg(test)]
mod tests;
