pub const DEFAULT_BRANCH_PREFIX: &str = "agent/";
pub const DEFAULT_WORKTREE_DIR: &str = ".worktrees";
pub const DEFAULT_AGENT: &str = "claude";
pub const DEFAULT_MODE: &str = "default";

pub const AGENT_OPTION: &str = "@agent-sidebar-default-agent";
pub const BRANCH_PREFIX_OPTION: &str = "@agent-sidebar-branch-prefix";
pub const WORKTREE_DIR_OPTION: &str = "@agent-sidebar-worktree-dir";

pub const AGENTS: &[&str] = &["claude", "codex", "opencode", "pi"];
pub const CLAUDE_MODES: &[&str] = &[
    "default",
    "plan",
    "acceptEdits",
    "dontAsk",
    "bypassPermissions",
];
pub const CODEX_MODES: &[&str] = &["default", "auto", "bypassPermissions"];
pub const OPENCODE_MODES: &[&str] = &["default"];
pub const PI_MODES: &[&str] = &["default"];

pub fn modes_for(agent: &str) -> &'static [&'static str] {
    match agent {
        "codex" => CODEX_MODES,
        "opencode" => OPENCODE_MODES,
        "pi" => PI_MODES,
        _ => CLAUDE_MODES,
    }
}

/// Compose the shell command to run inside the new pane from `agent`
/// and `mode`. Unsupported combinations fall back to launching the
/// agent with no flags.
pub fn agent_command(agent: &str, mode: &str) -> String {
    match (agent, mode) {
        ("claude", "" | "default") => "claude".into(),
        // Only forward modes that Claude Code's `--permission-mode`
        // actually accepts; an unrecognised value (e.g. a stale tmux
        // option from an old build) would otherwise make `claude` exit
        // with an argument error instead of starting up.
        ("claude", m @ ("plan" | "acceptEdits" | "dontAsk" | "bypassPermissions")) => {
            format!("claude --permission-mode {m}")
        }
        ("claude", _) => "claude".into(),
        ("codex", "auto") => "codex --full-auto".into(),
        ("codex", "bypassPermissions") => "codex --dangerously-bypass-approvals-and-sandbox".into(),
        ("codex", _) => "codex".into(),
        ("opencode", _) => "opencode".into(),
        ("pi", _) => "pi".into(),
        (a, _) => a.to_string(),
    }
}

/// How much to clean up when the user presses `x` on a spawn-created pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveMode {
    /// Only `tmux kill-window`. The git worktree and branch stay.
    WindowOnly,
    /// Kill the window AND `git worktree remove --force`.
    WindowAndWorktree,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_for_claude_returns_claude_modes_by_default() {
        assert_eq!(modes_for("claude"), CLAUDE_MODES);
    }

    #[test]
    fn modes_for_codex_returns_codex_modes() {
        assert_eq!(modes_for("codex"), CODEX_MODES);
    }

    #[test]
    fn modes_for_opencode_returns_opencode_modes() {
        assert_eq!(modes_for("opencode"), OPENCODE_MODES);
    }

    #[test]
    fn modes_for_pi_returns_pi_modes() {
        assert_eq!(modes_for("pi"), PI_MODES);
    }

    #[test]
    fn modes_for_unknown_agent_falls_back_to_claude_modes() {
        assert_eq!(modes_for("gemini"), CLAUDE_MODES);
        assert_eq!(modes_for(""), CLAUDE_MODES);
    }

    #[test]
    fn agent_command_claude_variants() {
        assert_eq!(agent_command("claude", ""), "claude");
        assert_eq!(agent_command("claude", "default"), "claude");
        assert_eq!(
            agent_command("claude", "plan"),
            "claude --permission-mode plan"
        );
        assert_eq!(
            agent_command("claude", "acceptEdits"),
            "claude --permission-mode acceptEdits"
        );
    }

    #[test]
    fn agent_command_codex_variants() {
        assert_eq!(agent_command("codex", "default"), "codex");
        assert_eq!(agent_command("codex", ""), "codex");
        assert_eq!(agent_command("codex", "auto"), "codex --full-auto");
        assert_eq!(
            agent_command("codex", "bypassPermissions"),
            "codex --dangerously-bypass-approvals-and-sandbox"
        );
    }

    #[test]
    fn agent_command_opencode_ignores_mode() {
        // OpenCode currently exposes only the default mode; every variant
        // collapses to the bare binary.
        assert_eq!(agent_command("opencode", "default"), "opencode");
        assert_eq!(agent_command("opencode", "plan"), "opencode");
        assert_eq!(agent_command("opencode", ""), "opencode");
    }

    #[test]
    fn agent_command_pi_ignores_mode() {
        assert_eq!(agent_command("pi", "default"), "pi");
        assert_eq!(agent_command("pi", "plan"), "pi");
        assert_eq!(agent_command("pi", ""), "pi");
    }

    #[test]
    fn agent_command_unknown_agent_passes_through() {
        assert_eq!(agent_command("gemini", "default"), "gemini");
    }

    #[test]
    fn agent_command_claude_unknown_mode_falls_back_to_bare_claude() {
        // Regression: an unknown / stale Claude mode (e.g. persisted
        // from an older build whose mode list has since been removed)
        // used to be forwarded verbatim into `--permission-mode`,
        // making `claude` exit with an argument error on spawn. The
        // documented contract is "fall back to launching the agent
        // with no flags"; this test pins that behaviour.
        assert_eq!(agent_command("claude", "unknown"), "claude");
        assert_eq!(agent_command("claude", "legacy-mode-from-v0"), "claude");
    }

    #[test]
    fn agent_command_claude_all_whitelisted_modes_forwarded() {
        // Every mode the popup offers must still round-trip through
        // `--permission-mode` after the whitelist was introduced.
        for mode in CLAUDE_MODES {
            let got = agent_command("claude", mode);
            let expected = match *mode {
                "default" => "claude".to_string(),
                m => format!("claude --permission-mode {m}"),
            };
            assert_eq!(got, expected, "mode {mode} must match doc contract");
        }
    }

    #[test]
    fn agents_list_matches_modes_for_dispatch() {
        // Every agent listed in the public AGENTS catalog must resolve
        // to a non-empty modes list, otherwise the popup's mode cycle
        // would divide by zero.
        for agent in AGENTS {
            assert!(
                !modes_for(agent).is_empty(),
                "agent {agent} returned an empty mode list"
            );
        }
    }
}
