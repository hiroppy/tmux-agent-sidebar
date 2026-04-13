//! Read Claude Code's own plugin install registry to detect whether
//! tmux-agent-sidebar has been installed as a Claude Code plugin.
//!
//! Claude Code maintains `~/.claude/plugins/installed_plugins.json` —
//! a JSON catalog keyed by `<plugin>@<marketplace>` whose value is a
//! list of installs (one per scope). We read it once at sidebar
//! startup so the TUI can:
//!
//! 1. Suppress the "missing hooks" notice for Claude — the plugin
//!    guarantees the hooks are wired up, so the user-side
//!    `~/.claude/settings.json` is allowed to be empty.
//! 2. Surface a "plugin out of date" notice when the recorded version
//!    differs from the running binary's version, so the user knows to
//!    restart Claude Code (which causes the plugin to reload).
//!
//! Reading Claude Code's own registry (instead of bridging through a
//! TMPDIR file written by hook subprocesses) means uninstalls are
//! detected immediately on the next sidebar restart, with no stale
//! state to clean up. Every failure path is silent — missing file,
//! malformed JSON, schema drift all degrade to "plugin not installed".

use std::fs;
use std::path::{Path, PathBuf};

const PLUGIN_NAME: &str = "tmux-agent-sidebar";
const RESIDUAL_HOOK_NEEDLE: &str = "tmux-agent-sidebar/hook.sh";

/// Return the version of the `tmux-agent-sidebar` plugin recorded in
/// Claude Code's `installed_plugins.json`, or `None` when the plugin is
/// not installed (or the registry cannot be read). Resolved once at
/// sidebar startup; see `version_notice` for the analogous pattern.
pub fn installed_plugin_version() -> Option<String> {
    installed_plugin_version_from(&claude_plugins_registry_path()?)
}

fn claude_plugins_registry_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".claude/plugins/installed_plugins.json"))
}

fn installed_plugin_version_from(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let plugins = json.get("plugins")?.as_object()?;
    for (key, installs) in plugins {
        // Match by plugin name (the part before `@`), not by full key —
        // a future marketplace rename or republish from a different
        // marketplace should still resolve to the installed plugin.
        let name = key.split('@').next().unwrap_or("");
        if name != PLUGIN_NAME {
            continue;
        }
        if let Some(version) = installs.as_array().and_then(|installs| {
            installs.iter().find_map(|install| {
                install
                    .get("version")
                    .and_then(|v| v.as_str())
                    .filter(|v| !v.is_empty())
            })
        }) {
            return Some(version.to_string());
        }
    }
    None
}

/// Whether the user's `~/.claude/settings.json` still contains residual
/// `tmux-agent-sidebar/hook.sh` entries from the legacy manual setup.
///
/// When this returns `true` AND the plugin is also installed, every hook
/// fires twice — once via the plugin and once via the user's manual
/// setting. The notices popup needs to surface this so the user can
/// clean up the duplicates. Resolved once at sidebar startup, matching
/// the `installed_plugin_version()` pattern.
pub fn claude_settings_has_residual_hooks() -> bool {
    claude_settings_has_residual_hooks_at(&claude_settings_path())
}

fn claude_settings_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join(".claude/settings.json")
}

fn claude_settings_has_residual_hooks_at(path: &Path) -> bool {
    let Ok(raw) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return false;
    };
    let Some(hooks) = json.get("hooks").and_then(|v| v.as_object()) else {
        return false;
    };
    hooks
        .values()
        .filter_map(|v| v.as_array())
        .flatten()
        .filter_map(|matcher_obj| matcher_obj.get("hooks").and_then(|h| h.as_array()))
        .flatten()
        .filter_map(|action| action.get("command").and_then(|c| c.as_str()))
        .any(|cmd| cmd.contains(RESIDUAL_HOOK_NEEDLE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_registry(label: &str) -> PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("tmux-as-installed-plugins-{label}-{id}.json"));
        let _ = fs::remove_file(&path);
        path
    }

    fn write_registry(path: &Path, body: &str) {
        fs::write(path, body).unwrap();
    }

    #[test]
    fn returns_version_when_plugin_is_installed() {
        let path = unique_registry("installed");
        write_registry(
            &path,
            r#"{
                "version": 2,
                "plugins": {
                    "tmux-agent-sidebar@hiroppy": [
                        {"scope":"user","installPath":"/x","version":"0.5.0"}
                    ]
                }
            }"#,
        );
        assert_eq!(installed_plugin_version_from(&path), Some("0.5.0".into()));
    }

    #[test]
    fn returns_none_when_plugin_not_in_registry() {
        // Other plugins exist but tmux-agent-sidebar is uninstalled →
        // notice should re-appear without any stale TMPDIR state to
        // clean up.
        let path = unique_registry("not-installed");
        write_registry(
            &path,
            r#"{
                "version": 2,
                "plugins": {
                    "code-review@anthropic": [
                        {"scope":"user","version":"1.0.0"}
                    ]
                }
            }"#,
        );
        assert_eq!(installed_plugin_version_from(&path), None);
    }

    #[test]
    fn returns_none_when_registry_file_missing() {
        let path = unique_registry("missing");
        // Note: file deliberately not written.
        assert_eq!(installed_plugin_version_from(&path), None);
    }

    #[test]
    fn returns_none_when_registry_is_garbage() {
        let path = unique_registry("garbage");
        write_registry(&path, "not-json");
        assert_eq!(installed_plugin_version_from(&path), None);
    }

    #[test]
    fn returns_none_when_plugins_field_missing() {
        let path = unique_registry("no-plugins-field");
        write_registry(&path, r#"{"version": 2}"#);
        assert_eq!(installed_plugin_version_from(&path), None);
    }

    #[test]
    fn returns_none_when_install_array_is_empty() {
        let path = unique_registry("empty-installs");
        write_registry(
            &path,
            r#"{"version":2,"plugins":{"tmux-agent-sidebar@hiroppy":[]}}"#,
        );
        assert_eq!(installed_plugin_version_from(&path), None);
    }

    #[test]
    fn matches_plugin_regardless_of_marketplace_suffix() {
        // Future-proof against republishing under a different
        // marketplace name (e.g. an official anthropic registry).
        let path = unique_registry("different-marketplace");
        write_registry(
            &path,
            r#"{
                "version": 2,
                "plugins": {
                    "tmux-agent-sidebar@somewhere-else": [
                        {"scope":"user","version":"0.6.0"}
                    ]
                }
            }"#,
        );
        assert_eq!(installed_plugin_version_from(&path), Some("0.6.0".into()));
    }

    #[test]
    fn returns_none_when_version_field_is_empty_string() {
        let path = unique_registry("empty-version");
        write_registry(
            &path,
            r#"{
                "version": 2,
                "plugins": {
                    "tmux-agent-sidebar@hiroppy": [
                        {"scope":"user","version":""}
                    ]
                }
            }"#,
        );
        assert_eq!(installed_plugin_version_from(&path), None);
    }

    #[test]
    fn returns_first_non_empty_version_across_multiple_installs() {
        let path = unique_registry("multiple-installs");
        write_registry(
            &path,
            r#"{
                "version": 2,
                "plugins": {
                    "tmux-agent-sidebar@hiroppy": [
                        {"scope":"user","installPath":"/x","version":""},
                        {"scope":"project","installPath":"/y","version":"0.6.0"}
                    ]
                }
            }"#,
        );
        assert_eq!(installed_plugin_version_from(&path), Some("0.6.0".into()));
    }

    // ─── claude_settings_has_residual_hooks_at ───────────────────────

    fn unique_settings(label: &str) -> PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!("tmux-as-claude-settings-{label}-{id}.json"));
        let _ = fs::remove_file(&path);
        path
    }

    #[test]
    fn residual_hooks_false_when_settings_file_missing() {
        let path = unique_settings("missing");
        assert!(!claude_settings_has_residual_hooks_at(&path));
    }

    #[test]
    fn residual_hooks_false_when_settings_file_has_no_hooks_object() {
        let path = unique_settings("no-hooks-object");
        fs::write(&path, r#"{"theme":"dark"}"#).unwrap();
        assert!(!claude_settings_has_residual_hooks_at(&path));
    }

    #[test]
    fn residual_hooks_false_when_no_command_mentions_tmux_agent_sidebar() {
        let path = unique_settings("clean");
        fs::write(
            &path,
            r#"{
                "hooks": {
                    "SessionStart": [
                        {"matcher":"","hooks":[{"type":"command","command":"echo hi"}]}
                    ]
                }
            }"#,
        )
        .unwrap();
        assert!(!claude_settings_has_residual_hooks_at(&path));
    }

    #[test]
    fn residual_hooks_true_when_legacy_command_present() {
        // The exact shape the project's legacy README told users to
        // paste into `~/.claude/settings.json`. After a plugin install
        // these entries cause every hook to fire twice — the notices
        // popup must keep flagging Claude until they are removed.
        let path = unique_settings("residual");
        fs::write(
            &path,
            r#"{
                "hooks": {
                    "SessionStart": [
                        {"matcher":"","hooks":[{"type":"command","command":"bash ~/.tmux/plugins/tmux-agent-sidebar/hook.sh claude session-start"}]}
                    ],
                    "PostToolUse": [
                        {"matcher":"","hooks":[{"type":"command","command":"bash ~/.tmux/plugins/tmux-agent-sidebar/hook.sh claude activity-log"}]}
                    ]
                }
            }"#,
        )
        .unwrap();
        assert!(claude_settings_has_residual_hooks_at(&path));
    }

    #[test]
    fn residual_hooks_true_when_only_one_legacy_command_present() {
        // Even a single leftover entry causes a duplicate hook fire.
        let path = unique_settings("residual-one");
        fs::write(
            &path,
            r#"{
                "hooks": {
                    "Stop": [
                        {"matcher":"","hooks":[{"type":"command","command":"bash /custom/path/tmux-agent-sidebar/hook.sh claude stop"}]}
                    ]
                }
            }"#,
        )
        .unwrap();
        assert!(claude_settings_has_residual_hooks_at(&path));
    }

    #[test]
    fn residual_hooks_false_when_settings_is_garbage() {
        let path = unique_settings("garbage");
        fs::write(&path, "not-json").unwrap();
        assert!(!claude_settings_has_residual_hooks_at(&path));
    }
}
