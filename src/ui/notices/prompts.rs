use crate::tmux::{CLAUDE_AGENT, CODEX_AGENT};

/// Build the ready-to-paste LLM prompt for the given agent name.
///
/// - **Claude**: emits a *migration* prompt that asks the LLM to delete any
///   existing `tmux-agent-sidebar/hook.sh` entries from the user's
///   `~/.claude/settings.json` and then point the user at `/plugin install`.
///   This is the only supported wiring path going forward — bundled
///   hooks via the Claude Code plugin manifest. The prompt embeds the
///   plugin root resolved from the running binary so users with custom
///   install layouts get a working install path. If `current_exe()`
///   fails, the prompt still works because the plugin root falls back
///   to the canonical TPM path.
/// - **Codex**: emits the legacy `setup codex` prompt because Codex CLI
///   has no plugin mechanism upstream. This branch genuinely needs the
///   running executable path, so it returns `None` if `current_exe()`
///   cannot be resolved.
///
/// Returns `None` for unknown agents.
pub(crate) fn prompt_for_agent(agent: &str) -> Option<String> {
    match agent {
        CLAUDE_AGENT => Some(build_claude_migration_prompt(
            plugin_root_from_exe().as_deref(),
        )),
        CODEX_AGENT => {
            // Shell-quote the path so an install location containing
            // spaces (e.g. macOS `/Applications/tmux-agent-sidebar/…`)
            // still yields a runnable command when the user pastes
            // this prompt into their shell.
            let exe_path =
                crate::cli::setup::shell_quote(&std::env::current_exe().ok()?.to_string_lossy());
            Some(format!(
                "Run {exe_path} setup codex. Before pasting the hooks, make sure \
                 ~/.codex/config.toml contains:\n\
                 \n\
                 [features]\n\
                 codex_hooks = true\n\
                 \n\
                 Add these hooks to ~/.codex/hooks.json. If hooks already \
                 exist, merge them without making destructive changes. Restart \
                 Codex after changing config.toml so the feature flag takes effect."
            ))
        }
        _ => None,
    }
}

/// Walk up from the running binary looking for `.claude-plugin/plugin.json`,
/// matching the install layouts supported elsewhere in the project
/// (`<plugin>/bin/tmux-agent-sidebar` and
/// `<plugin>/target/release/tmux-agent-sidebar`). Shares the upward-walk
/// loop with `cli::setup::resolve_hook_script`.
fn plugin_root_from_exe() -> Option<String> {
    crate::cli::setup::walk_up_from_exe(3, |dir| {
        dir.join(".claude-plugin")
            .join("plugin.json")
            .is_file()
            .then(|| dir.to_string_lossy().into_owned())
    })
}

/// Collapse an absolute path to a `~`-prefixed form when it lives
/// under the user's home directory. Used to make the migration prompt
/// portable across machines: `/Users/hiroppy/.tmux/plugins/...`
/// renders as `~/.tmux/plugins/...` so a screenshot or copy-paste
/// from one user does not bake in another user's literal home path.
fn tildify(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) => tildify_with_home(path, &home),
        Err(_) => path.to_string(),
    }
}

fn tildify_with_home(path: &str, home: &str) -> String {
    if home.is_empty() {
        return path.to_string();
    }
    if path == home {
        return "~".to_string();
    }
    if let Some(rest) = path.strip_prefix(home).and_then(|s| s.strip_prefix('/')) {
        return format!("~/{}", rest);
    }
    path.to_string()
}

/// Compose the LLM migration prompt for Claude Code users. `plugin_root`
/// is `Some` when the binary lives next to a `.claude-plugin/plugin.json`
/// (the common case), in which case the prompt names that exact path so
/// the user can paste a runnable `/plugin marketplace add` command. The
/// resolved path is tilde-collapsed when it lives under the user's home
/// so the rendered command stays portable (and fits narrower sidebars).
/// The fallback is the canonical TPM install path documented in the
/// README.
fn build_claude_migration_prompt(plugin_root: Option<&str>) -> String {
    let marketplace_path = plugin_root
        .map(tildify)
        .unwrap_or_else(|| "~/.tmux/plugins/tmux-agent-sidebar".to_string());
    format!(
        "Migrate this user from the manual ~/.claude/settings.json setup to the \
         tmux-agent-sidebar Claude Code plugin:\n\
         \n\
         1. Edit ~/.claude/settings.json and remove every \"command\" entry whose \
         value contains \"tmux-agent-sidebar/hook.sh\" from each \"hooks\" section. \
         Clean up any \"hooks\" arrays that become empty (drop the trigger key) and \
         remove the top-level \"hooks\" object if it becomes empty. If no such \
         entries exist, skip this step silently.\n\
         \n\
         2. Then tell the user verbatim:\n\
         \"Run these two commands in this Claude Code session, then restart \
         Claude Code so the bundled hooks take effect:\n\
         /plugin marketplace add {marketplace_path}\n\
         /plugin install tmux-agent-sidebar@hiroppy\""
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_for_agent_codex_uses_running_executable_path() {
        // Codex stays on the legacy `setup` flow because Codex CLI has
        // no plugin mechanism upstream.
        let exe = std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let codex = prompt_for_agent("codex").unwrap();
        assert!(
            codex.contains(&exe),
            "codex prompt missing current_exe path: {codex}"
        );
        assert!(codex.contains("setup codex"));
        assert!(codex.contains("~/.codex/config.toml"));
        assert!(codex.contains("~/.codex/hooks.json"));
        assert!(codex.contains("codex_hooks = true"));
        assert!(codex.contains("Restart Codex"));
    }

    #[test]
    fn prompt_for_agent_claude_is_a_migration_prompt() {
        // The Claude prompt must steer users toward the plugin install
        // and away from the legacy settings.json hook setup. It also
        // needs a concrete removal step so users currently on the manual
        // path get cleaned up before the plugin takes over.
        let claude = prompt_for_agent("claude").unwrap();
        // Claude Code's `/plugin install` does not accept local paths
        // directly; users must register a marketplace first. The prompt
        // therefore needs both `marketplace add` and `install` lines.
        assert!(
            claude.contains("/plugin marketplace add"),
            "claude prompt must surface the marketplace add command: {claude}"
        );
        assert!(
            claude.contains("/plugin install tmux-agent-sidebar@hiroppy"),
            "claude prompt must surface the plugin install command keyed to \
             the bundled marketplace name: {claude}"
        );
        assert!(
            claude.contains("~/.claude/settings.json"),
            "claude prompt must reference settings.json so the LLM knows \
             which file to clean up: {claude}"
        );
        assert!(
            claude.contains("tmux-agent-sidebar/hook.sh"),
            "claude prompt must tell the LLM exactly which existing entries \
             to remove: {claude}"
        );
        assert!(
            claude.contains("restart Claude Code"),
            "claude prompt must remind the user to restart so the plugin's \
             bundled hooks load: {claude}"
        );
        assert!(
            !claude.contains("setup claude"),
            "claude prompt must NOT recommend the legacy `setup claude` \
             flow anymore: {claude}"
        );
    }

    #[test]
    fn build_claude_migration_prompt_uses_resolved_plugin_root_when_available() {
        // Use a path that is guaranteed NOT to live under HOME so the
        // tildify pass cannot rewrite it on either the dev machine or
        // CI (where HOME varies). The tilde-collapse behavior is
        // covered directly by the `tildify_with_home` tests below.
        let prompt = build_claude_migration_prompt(Some("/opt/tmux-agent-sidebar"));
        assert!(prompt.contains("/opt/tmux-agent-sidebar"));
    }

    #[test]
    fn build_claude_migration_prompt_falls_back_to_canonical_path() {
        // No plugin root resolved → fall back to the README-documented
        // TPM install path so the pasted command is still runnable for
        // the typical user.
        let prompt = build_claude_migration_prompt(None);
        assert!(prompt.contains("~/.tmux/plugins/tmux-agent-sidebar"));
    }

    // ─── tildify_with_home ───────────────────────────────────────────

    #[test]
    fn tildify_collapses_paths_under_home_to_tilde() {
        assert_eq!(
            tildify_with_home(
                "/Users/alice/.tmux/plugins/tmux-agent-sidebar",
                "/Users/alice"
            ),
            "~/.tmux/plugins/tmux-agent-sidebar"
        );
    }

    #[test]
    fn tildify_returns_lone_tilde_when_path_is_home() {
        assert_eq!(tildify_with_home("/Users/alice", "/Users/alice"), "~");
    }

    #[test]
    fn tildify_leaves_paths_outside_home_unchanged() {
        assert_eq!(
            tildify_with_home("/opt/tmux-agent-sidebar", "/Users/alice"),
            "/opt/tmux-agent-sidebar"
        );
    }

    #[test]
    fn tildify_does_not_collapse_a_prefix_that_only_shares_a_path_segment() {
        // `/Users/aliceother/x` must not collapse against `/Users/alice`.
        // The strip is gated on the trailing `/` so partial-name matches
        // do not produce nonsense like `~other/x`.
        assert_eq!(
            tildify_with_home("/Users/aliceother/x", "/Users/alice"),
            "/Users/aliceother/x"
        );
    }

    #[test]
    fn tildify_no_op_when_home_is_empty() {
        // Defensive: an empty HOME (set but blank) must not collapse
        // every absolute path to `~/...`.
        assert_eq!(tildify_with_home("/Users/alice/x", ""), "/Users/alice/x");
    }

    #[test]
    fn prompt_for_agent_none_for_unknown_agent() {
        assert_eq!(prompt_for_agent("gemini"), None);
        assert_eq!(prompt_for_agent(""), None);
    }

    #[test]
    fn prompt_for_agent_codex_shell_quotes_executable_path() {
        // Regression: the Codex prompt is rendered as ready-to-paste
        // shell text, so an install path with whitespace (or any
        // metacharacter) must go through the same `shell_quote`
        // helper that the setup writer uses. Without quoting, a user
        // who installs the plugin under `/Applications/tmux agent/`
        // gets a broken command when they paste the prompt.
        //
        // We can't control `std::env::current_exe()` in tests, so
        // the assertion compares against the quoted form of whatever
        // the test binary path happens to be — the point is that
        // the emitted prompt mirrors the canonical quoting helper.
        let exe = std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let quoted = crate::cli::setup::shell_quote(&exe);
        let codex = prompt_for_agent("codex").unwrap();
        assert!(
            codex.contains(&format!("Run {quoted} setup codex")),
            "codex prompt should embed `shell_quote(current_exe())`: \
             quoted={quoted}, prompt={codex}"
        );
    }
}
