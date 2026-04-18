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
//! 2. Surface a "plugin out of date" notice when the `hooks/hooks.json`
//!    snapshot in Claude's plugin cache differs from the one embedded
//!    into this binary at build time, so the user knows to run
//!    `/plugin update` to pick up new hook declarations.
//!
//! Why compare `hooks/hooks.json` instead of the plugin `version`
//! string: `hook.sh` is a thin wrapper that always dispatches to the
//! latest-built binary on disk, so a bare version mismatch between the
//! running binary and the cached manifest is meaningless — the binary
//! already runs the newest code regardless of which cache directory
//! holds `hook.sh`. The one thing that *does* require `/plugin update`
//! is a change to the hook declarations (new events, matcher tweaks),
//! and those live in `hooks/hooks.json`. Embedding that file via
//! `include_str!` makes the binary self-describing — no need to go
//! hunting for the marketplace source tree to find the "live" copy.
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

/// Files that ship with *this* binary, snapshotted at compile time via
/// `include_str!`, paired with their path relative to the plugin root.
/// Any mismatch — or absence — against the copy in Claude's plugin
/// cache means the cache is behind (or corrupt) and the user needs
/// `/plugin update` so Claude Code re-reads the affected files.
///
/// Tracked entries are scoped to files whose content changes alter
/// runtime behavior:
/// - `hook.sh` — the wrapper Claude invokes for every hook event. A
///   change here (new fallback paths, error handling) only reaches
///   users after `/plugin update` refreshes their cache.
/// - `hooks/hooks.json` — declares which Claude hook events route to
///   `hook.sh`. Adding or renaming events requires a cache refresh
///   before Claude Code wires the new events up.
///
/// `.claude-plugin/plugin.json` is deliberately excluded: its only
/// per-release churn is the `version` field, and firing the Stale
/// notice on every cosmetic version bump would reintroduce the
/// false-positive noise this design was meant to kill. A release that
/// only bumps the version number has no functional drift Claude Code
/// needs to re-read.
///
/// Kept as a slice so adding a future file (agent / command markdown,
/// MCP config) is a one-line change and the comparison semantics —
/// "outdated iff *any* tracked file differs or is missing" — stay
/// uniform.
const EMBEDDED_PLUGIN_FILES: &[(&str, &str)] = &[
    ("hook.sh", include_str!("../../hook.sh")),
    ("hooks/hooks.json", include_str!("../../hooks/hooks.json")),
];

/// Lifetime state of the Claude Code plugin install, resolved once at
/// sidebar startup. All fields default to "plugin not installed".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClaudePluginStatus {
    /// Whether `tmux-agent-sidebar` is recorded in Claude Code's
    /// `installed_plugins.json`. Derived from the presence of a matching
    /// entry with a non-empty install path.
    pub installed: bool,
    /// True when the plugin is installed AND at least one of the files
    /// listed in [`EMBEDDED_PLUGIN_FILES`] differs between its cache
    /// (resolved via `installPath` from the registry) and the copy
    /// embedded in this binary. Only meaningful alongside
    /// `installed: true`; remains `false` for every "not installed"
    /// path so consumers can branch on `installed` first.
    pub cache_outdated: bool,
}

/// Resolve the Claude plugin install status from the user's
/// `~/.claude/plugins/installed_plugins.json`. Registry-level failure
/// paths (missing registry, unreadable file, malformed JSON, missing
/// install path) degrade to "plugin not installed" — the notice popup
/// is advisory, never blocking. Once the install path resolves, any
/// missing / divergent tracked file surfaces as `cache_outdated`.
pub fn installed_plugin_status() -> ClaudePluginStatus {
    let Some(registry) = claude_plugins_registry_path() else {
        return ClaudePluginStatus::default();
    };
    installed_plugin_status_from(&registry, EMBEDDED_PLUGIN_FILES)
}

fn claude_plugins_registry_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".claude/plugins/installed_plugins.json"))
}

fn installed_plugin_status_from(
    registry_path: &Path,
    embedded_files: &[(&str, &str)],
) -> ClaudePluginStatus {
    match installed_plugin_install_path_from(registry_path) {
        Some(install_path) => ClaudePluginStatus {
            installed: true,
            cache_outdated: cache_files_outdated(&install_path, embedded_files),
        },
        None => ClaudePluginStatus::default(),
    }
}

/// Extract the recorded `installPath` for the `tmux-agent-sidebar`
/// plugin, preferring the first entry that carries a non-empty path.
/// Returns `None` when the plugin is not installed or the registry is
/// malformed/unreadable.
fn installed_plugin_install_path_from(path: &Path) -> Option<PathBuf> {
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
        if let Some(install_path) = installs.as_array().and_then(|installs| {
            installs.iter().find_map(|install| {
                install
                    .get("installPath")
                    .and_then(|v| v.as_str())
                    .filter(|v| !v.is_empty())
                    .map(PathBuf::from)
            })
        }) {
            return Some(install_path);
        }
    }
    None
}

/// Compare each cached plugin file against the embedded copy baked in
/// at build time. Returns `true` as soon as **any** file differs or
/// cannot be read, since either condition warrants `/plugin update`
/// (or a reinstall).
///
/// Every entry in [`EMBEDDED_PLUGIN_FILES`] is a file the plugin is
/// guaranteed to ship, so a read error (missing, permission denied,
/// corrupt symlink) means the cache is partial or broken — exactly the
/// state the user needs to be nagged about. Byte-exact comparison is
/// intentional: tracked files only diverge during a release, so
/// surfacing incidental whitespace edits is acceptable (running
/// `/plugin update` resolves them cleanly).
fn cache_files_outdated(install_path: &Path, embedded_files: &[(&str, &str)]) -> bool {
    embedded_files.iter().any(|(rel_path, expected)| {
        match fs::read_to_string(install_path.join(rel_path)) {
            Ok(cached) => cached != *expected,
            Err(_) => true,
        }
    })
}

/// Whether the user's `~/.claude/settings.json` still contains residual
/// `tmux-agent-sidebar/hook.sh` entries from the legacy manual setup.
///
/// When this returns `true` AND the plugin is also installed, every hook
/// fires twice — once via the plugin and once via the user's manual
/// setting. The notices popup needs to surface this so the user can
/// clean up the duplicates. Resolved once at sidebar startup, matching
/// the `installed_plugin_status()` pattern.
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

    // ─── EMBEDDED_PLUGIN_FILES shape ─────────────────────────────────
    //
    // Guard rails against an `include_str!` pointing at the wrong file
    // (e.g. a placeholder, a renamed artifact, or a build-time swap).
    // These checks cost a few microseconds at test time and let a
    // mis-wired tracked file fail loudly instead of silently shipping
    // a stale-notice detector that can never fire.

    #[test]
    fn embedded_plugin_files_is_non_empty() {
        assert!(
            !EMBEDDED_PLUGIN_FILES.is_empty(),
            "EMBEDDED_PLUGIN_FILES is empty — `cache_files_outdated` would \
             never flag a cache as stale, silently disabling the Stale notice"
        );
    }

    #[test]
    fn embedded_plugin_files_have_no_empty_content() {
        // Every tracked file must carry real bytes. A blank payload
        // would match any cache file whose embedded copy someone
        // emptied by accident, masking real drift.
        for (rel_path, body) in EMBEDDED_PLUGIN_FILES {
            assert!(
                !body.trim().is_empty(),
                "embedded tracked file {rel_path:?} is empty — \
                 `include_str!` is pointing at the wrong artifact"
            );
        }
    }

    #[test]
    fn embedded_hook_sh_starts_with_shebang() {
        // The wrapper is invoked as `bash hook.sh ...` by
        // `hooks/hooks.json`, so a missing shebang is not a functional
        // failure — but its absence would also mean `include_str!`
        // grabbed the wrong file, which is the regression this guards.
        let (_, hook_sh) = EMBEDDED_PLUGIN_FILES
            .iter()
            .find(|(rel, _)| *rel == "hook.sh")
            .expect("hook.sh must stay in EMBEDDED_PLUGIN_FILES");
        assert!(
            hook_sh.starts_with("#!"),
            "embedded hook.sh does not start with a shebang — \
             `include_str!` is pointing at the wrong file"
        );
    }

    #[test]
    fn embedded_hooks_json_parses_with_hooks_object() {
        // Catches accidental newline / UTF-8 BOM drift introduced by
        // an editor — any such corruption would make every user's
        // cache look outdated the moment this binary ships.
        let (_, hooks_json) = EMBEDDED_PLUGIN_FILES
            .iter()
            .find(|(rel, _)| *rel == "hooks/hooks.json")
            .expect("hooks/hooks.json must stay in EMBEDDED_PLUGIN_FILES");
        let json: serde_json::Value =
            serde_json::from_str(hooks_json).expect("embedded hooks.json must parse");
        assert!(
            json.get("hooks").and_then(|v| v.as_object()).is_some(),
            "embedded hooks.json is missing a top-level `hooks` object"
        );
    }

    fn unique_dir(label: &str) -> PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("tmux-as-plugin-state-{label}-{id}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

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

    /// Write a fake Claude plugin cache with a `hooks/hooks.json` file
    /// containing `body`, and return its root path.
    fn write_cache_with_hooks(label: &str, body: &str) -> PathBuf {
        let root = unique_dir(label);
        fs::create_dir_all(root.join("hooks")).unwrap();
        fs::write(root.join("hooks/hooks.json"), body).unwrap();
        root
    }

    #[test]
    fn returns_install_path_when_plugin_is_installed() {
        let path = unique_registry("installed");
        write_registry(
            &path,
            r#"{
                "version": 2,
                "plugins": {
                    "tmux-agent-sidebar@hiroppy": [
                        {"scope":"user","installPath":"/opt/claude-cache/tmux-agent-sidebar/0.5.0","version":"0.5.0"}
                    ]
                }
            }"#,
        );
        assert_eq!(
            installed_plugin_install_path_from(&path),
            Some(PathBuf::from("/opt/claude-cache/tmux-agent-sidebar/0.5.0"))
        );
    }

    #[test]
    fn returns_none_when_plugin_not_in_registry() {
        let path = unique_registry("not-installed");
        write_registry(
            &path,
            r#"{
                "version": 2,
                "plugins": {
                    "code-review@anthropic": [
                        {"scope":"user","installPath":"/x","version":"1.0.0"}
                    ]
                }
            }"#,
        );
        assert_eq!(installed_plugin_install_path_from(&path), None);
    }

    #[test]
    fn returns_none_when_registry_file_missing() {
        let path = unique_registry("missing");
        assert_eq!(installed_plugin_install_path_from(&path), None);
    }

    #[test]
    fn returns_none_when_registry_is_garbage() {
        let path = unique_registry("garbage");
        write_registry(&path, "not-json");
        assert_eq!(installed_plugin_install_path_from(&path), None);
    }

    #[test]
    fn returns_none_when_plugins_field_missing() {
        let path = unique_registry("no-plugins-field");
        write_registry(&path, r#"{"version": 2}"#);
        assert_eq!(installed_plugin_install_path_from(&path), None);
    }

    #[test]
    fn returns_none_when_install_array_is_empty() {
        let path = unique_registry("empty-installs");
        write_registry(
            &path,
            r#"{"version":2,"plugins":{"tmux-agent-sidebar@hiroppy":[]}}"#,
        );
        assert_eq!(installed_plugin_install_path_from(&path), None);
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
                        {"scope":"user","installPath":"/tmp/elsewhere/0.6.0","version":"0.6.0"}
                    ]
                }
            }"#,
        );
        assert_eq!(
            installed_plugin_install_path_from(&path),
            Some(PathBuf::from("/tmp/elsewhere/0.6.0"))
        );
    }

    #[test]
    fn returns_none_when_install_path_is_empty_string() {
        let path = unique_registry("empty-install-path");
        write_registry(
            &path,
            r#"{
                "version": 2,
                "plugins": {
                    "tmux-agent-sidebar@hiroppy": [
                        {"scope":"user","installPath":""}
                    ]
                }
            }"#,
        );
        assert_eq!(installed_plugin_install_path_from(&path), None);
    }

    #[test]
    fn returns_first_non_empty_install_path_across_multiple_installs() {
        let path = unique_registry("multiple-installs");
        write_registry(
            &path,
            r#"{
                "version": 2,
                "plugins": {
                    "tmux-agent-sidebar@hiroppy": [
                        {"scope":"user","installPath":""},
                        {"scope":"project","installPath":"/project/0.6.0","version":"0.6.0"}
                    ]
                }
            }"#,
        );
        assert_eq!(
            installed_plugin_install_path_from(&path),
            Some(PathBuf::from("/project/0.6.0"))
        );
    }

    /// Write a file under the cache root at the given relative path,
    /// creating parent directories as needed.
    fn write_cache_file(root: &Path, rel_path: &str, body: &str) {
        let abs = root.join(rel_path);
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(abs, body).unwrap();
    }

    // ─── cache_files_outdated ────────────────────────────────────────

    #[test]
    fn files_match_when_single_cached_file_equals_embedded() {
        let root = write_cache_with_hooks("match", "{\"hooks\":{}}");
        assert!(!cache_files_outdated(
            &root,
            &[("hooks/hooks.json", "{\"hooks\":{}}")]
        ));
    }

    #[test]
    fn files_outdated_when_single_cached_file_differs_from_embedded() {
        let root = write_cache_with_hooks("differ", "{\"hooks\":{\"Old\":[]}}");
        assert!(cache_files_outdated(
            &root,
            &[("hooks/hooks.json", "{\"hooks\":{\"New\":[]}}")]
        ));
    }

    #[test]
    fn files_outdated_when_cached_file_is_missing() {
        // Every tracked file is one the plugin is guaranteed to ship.
        // Missing in the cache means the install is partial or broken,
        // so the Stale notice must surface so the user runs
        // `/plugin update` (or reinstalls).
        let root = unique_dir("missing-file");
        assert!(cache_files_outdated(
            &root,
            &[("hooks/hooks.json", "{\"hooks\":{}}")]
        ));
    }

    #[test]
    fn files_outdated_when_any_tracked_file_diverges() {
        // Validates the all-match semantics across multiple embedded
        // files: `hooks.json` is in sync but `.claude-plugin/plugin.json`
        // drifts, so the overall status must be "outdated".
        let root = unique_dir("multi-any-diverge");
        write_cache_file(&root, "hooks/hooks.json", "{\"hooks\":{}}");
        write_cache_file(
            &root,
            ".claude-plugin/plugin.json",
            "{\"version\":\"0.4.0\"}",
        );
        assert!(cache_files_outdated(
            &root,
            &[
                ("hooks/hooks.json", "{\"hooks\":{}}"),
                (".claude-plugin/plugin.json", "{\"version\":\"0.5.0\"}"),
            ]
        ));
    }

    #[test]
    fn files_not_outdated_when_all_tracked_files_match() {
        let root = unique_dir("multi-all-match");
        write_cache_file(&root, "hooks/hooks.json", "{\"hooks\":{}}");
        write_cache_file(
            &root,
            ".claude-plugin/plugin.json",
            "{\"version\":\"0.5.0\"}",
        );
        assert!(!cache_files_outdated(
            &root,
            &[
                ("hooks/hooks.json", "{\"hooks\":{}}"),
                (".claude-plugin/plugin.json", "{\"version\":\"0.5.0\"}"),
            ]
        ));
    }

    #[test]
    fn files_outdated_when_one_of_several_tracked_files_is_missing() {
        // Partial cache: one matching file, one absent. The missing
        // file alone is enough to flag the whole install as outdated.
        let root = unique_dir("multi-one-missing");
        write_cache_file(&root, "hooks/hooks.json", "{\"hooks\":{}}");
        // `.claude-plugin/plugin.json` is deliberately not written.
        assert!(cache_files_outdated(
            &root,
            &[
                ("hooks/hooks.json", "{\"hooks\":{}}"),
                (".claude-plugin/plugin.json", "{\"version\":\"0.5.0\"}"),
            ]
        ));
    }

    // ─── installed_plugin_status_from (composition) ──────────────────

    #[test]
    fn status_not_installed_when_registry_missing() {
        let path = unique_registry("status-missing");
        let status = installed_plugin_status_from(&path, &[("hooks/hooks.json", "ignored")]);
        assert_eq!(status, ClaudePluginStatus::default());
    }

    #[test]
    fn status_installed_and_in_sync_when_hooks_match() {
        let root = write_cache_with_hooks("status-match", "{\"hooks\":{}}");
        let registry = unique_registry("status-match-registry");
        write_registry(
            &registry,
            &format!(
                r#"{{"version":2,"plugins":{{"tmux-agent-sidebar@hiroppy":[{{"scope":"user","installPath":{:?}}}]}}}}"#,
                root.to_string_lossy()
            ),
        );
        let status =
            installed_plugin_status_from(&registry, &[("hooks/hooks.json", "{\"hooks\":{}}")]);
        assert_eq!(
            status,
            ClaudePluginStatus {
                installed: true,
                cache_outdated: false
            }
        );
    }

    #[test]
    fn status_installed_and_outdated_when_hooks_differ() {
        let root = write_cache_with_hooks("status-stale", "{\"hooks\":{\"Old\":[]}}");
        let registry = unique_registry("status-stale-registry");
        write_registry(
            &registry,
            &format!(
                r#"{{"version":2,"plugins":{{"tmux-agent-sidebar@hiroppy":[{{"scope":"user","installPath":{:?}}}]}}}}"#,
                root.to_string_lossy()
            ),
        );
        let status = installed_plugin_status_from(
            &registry,
            &[("hooks/hooks.json", "{\"hooks\":{\"New\":[]}}")],
        );
        assert_eq!(
            status,
            ClaudePluginStatus {
                installed: true,
                cache_outdated: true
            }
        );
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
