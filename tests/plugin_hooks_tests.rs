//! Drift detection between `hooks/hooks.json` (the Claude Code plugin
//! manifest shipped with this repo) and `ClaudeAdapter::HOOK_REGISTRATIONS`
//! (the in-code source of truth).
//!
//! When a Claude hook is added or removed in `src/adapter/claude.rs`, this
//! test fails until `hooks/hooks.json` is updated to match. Without it the
//! plugin would silently drift out of sync with the runtime adapter.

use std::collections::BTreeSet;
use std::fs;

use tmux_agent_sidebar::VERSION;
use tmux_agent_sidebar::adapter::claude::ClaudeAdapter;

const HOOKS_JSON_PATH: &str = "hooks/hooks.json";

fn load_hooks_json() -> serde_json::Value {
    let raw = fs::read_to_string(HOOKS_JSON_PATH)
        .unwrap_or_else(|e| panic!("failed to read {HOOKS_JSON_PATH}: {e}"));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("{HOOKS_JSON_PATH} is not valid JSON: {e}"))
}

#[test]
fn plugin_hooks_json_triggers_match_registration_table() {
    let json = load_hooks_json();
    let hooks = json
        .get("hooks")
        .and_then(|v| v.as_object())
        .expect("hooks/hooks.json must have a top-level `hooks` object");

    let actual: BTreeSet<String> = hooks.keys().cloned().collect();
    let expected: BTreeSet<String> = ClaudeAdapter::HOOK_REGISTRATIONS
        .iter()
        .map(|reg| reg.trigger.to_string())
        .collect();

    assert_eq!(
        actual, expected,
        "hooks/hooks.json triggers drifted from ClaudeAdapter::HOOK_REGISTRATIONS"
    );
}

#[test]
fn plugin_hooks_json_commands_use_expected_event_names() {
    let json = load_hooks_json();
    let hooks = json.get("hooks").and_then(|v| v.as_object()).unwrap();

    for reg in ClaudeAdapter::HOOK_REGISTRATIONS {
        let entries = hooks
            .get(reg.trigger)
            .and_then(|v| v.as_array())
            .unwrap_or_else(|| panic!("missing trigger {} in hooks.json", reg.trigger));
        assert_eq!(
            entries.len(),
            1,
            "trigger {} should have exactly one entry",
            reg.trigger
        );

        let inner = entries[0]
            .get("hooks")
            .and_then(|v| v.as_array())
            .unwrap_or_else(|| panic!("trigger {} entry missing inner hooks array", reg.trigger));
        assert_eq!(inner.len(), 1);

        let cmd = inner[0]
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("trigger {} command must be a string", reg.trigger));

        let expected_event = reg.kind.external_name();
        let expected_suffix = format!("claude {expected_event}");
        assert!(
            cmd.ends_with(&expected_suffix),
            "trigger {}: command {:?} does not end with {:?}",
            reg.trigger,
            cmd,
            expected_suffix
        );
        // The expansion MUST be quoted so plugin checkouts living
        // under paths with spaces or shell metacharacters are not
        // word-split by `bash` before `hook.sh` even runs. See PR #18
        // adversarial review (Codex) for the failure mode.
        assert!(
            cmd.contains("\"${CLAUDE_PLUGIN_ROOT}/hook.sh\""),
            "trigger {}: command {:?} must reference \
             `\"${{CLAUDE_PLUGIN_ROOT}}/hook.sh\"` (with the expansion \
             wrapped in double quotes) so paths with spaces survive",
            reg.trigger,
            cmd
        );
    }
}

fn load_plugin_manifest() -> serde_json::Value {
    let raw = fs::read_to_string(".claude-plugin/plugin.json")
        .expect("failed to read .claude-plugin/plugin.json");
    serde_json::from_str(&raw).expect(".claude-plugin/plugin.json is not valid JSON")
}

#[test]
fn plugin_manifest_omits_redundant_hooks_field() {
    // Claude Code auto-loads `hooks/hooks.json` from the plugin root. The
    // optional `hooks` field in `plugin.json` is for *additional* hook
    // files only — pointing it back at the standard path triggers a
    // "Duplicate hooks file detected" load error and the entire bundle
    // gets dropped.
    let json = load_plugin_manifest();

    assert_eq!(
        json.get("name").and_then(|v| v.as_str()),
        Some("tmux-agent-sidebar")
    );
    assert!(
        json.get("hooks").is_none(),
        "plugin.json must NOT declare a `hooks` field — the standard \
         hooks/hooks.json is auto-loaded; declaring it again causes \
         Claude Code to abort the plugin load with a duplicate-file error"
    );
}

#[test]
fn plugin_manifest_version_matches_cargo_toml() {
    // Claude Code uses `plugin.json`'s `version` field for update detection
    // (`/plugin update` and the marketplace cache compare against this).
    // If we bump Cargo.toml without bumping plugin.json, marketplace users
    // never see the new release. This test forces the two to stay in lockstep.
    let json = load_plugin_manifest();
    let plugin_version = json
        .get("version")
        .and_then(|v| v.as_str())
        .expect(".claude-plugin/plugin.json must declare a `version`");

    assert_eq!(
        plugin_version, VERSION,
        ".claude-plugin/plugin.json version ({plugin_version}) does not match \
         Cargo.toml version ({VERSION}). Bump both together — see \
         .claude/skills/version-release/SKILL.md."
    );
}
