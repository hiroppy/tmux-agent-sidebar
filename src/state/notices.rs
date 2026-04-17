use std::time::Instant;

/// Sub-state for the ⓘ notices popup, lifted out of [`AppState`] so its
/// seven related fields (button column, missing-hook groups, plugin
/// version, legacy hook flag, plugin notice, copy targets, copy feedback)
/// travel as a single unit.
#[derive(Debug, Clone, Default)]
pub struct NoticesState {
    /// Column of the ⓘ button in the secondary header, or `None` when the
    /// button is hidden. Used for click hit-testing.
    pub button_col: Option<u16>,
    /// Missing hooks grouped per agent, shown in the "Missing hooks"
    /// section of the popup.
    pub missing_hook_groups: Vec<NoticesMissingHookGroup>,
    /// Version of the `tmux-agent-sidebar` Claude Code plugin install
    /// detected at sidebar startup, or `None` when the plugin is not
    /// installed. Resolved once from
    /// `~/.claude/plugins/installed_plugins.json` and cached for the
    /// lifetime of the TUI process — restart the sidebar after a
    /// `/plugin install` or `/plugin uninstall` to pick up the change.
    /// `claude_plugin_notice` and the missing-hooks Claude filter are
    /// derived from this field.
    pub claude_plugin_installed_version: Option<String>,
    /// Whether `~/.claude/settings.json` still contains residual
    /// `tmux-agent-sidebar/hook.sh` entries from the legacy manual
    /// setup. Resolved once at startup. When this is `true` AND the
    /// plugin is installed, every hook fires twice and the popup must
    /// keep nagging the user to clean up.
    pub claude_settings_has_residual_hooks: bool,
    /// Drives the `Plugin / claude` section in the notices popup. See
    /// [`ClaudePluginNotice`] for the full set of variants. Derived from
    /// `claude_plugin_installed_version` in `refresh_notices`.
    pub claude_plugin_notice: Option<ClaudePluginNotice>,
    /// Click regions for the `copy` label on each agent row in the popup.
    pub copy_targets: Vec<NoticesCopyTarget>,
    /// Agent name and timestamp of the most recent successful copy, shown
    /// as a transient `copied` label next to the popup title.
    pub copied_at: Option<(String, Instant)>,
}

/// Missing hooks grouped by agent name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NoticesMissingHookGroup {
    pub agent: String,
    pub hooks: Vec<String>,
}

/// Notice surfaced in the popup's `Plugin / claude` section. The
/// variants are mutually exclusive and ordered by urgency:
/// `DuplicateHooks` > `InstallRecommended` > `Stale`. When the plugin
/// is installed, current, and the user has no residual manual hook
/// entries, no notice is set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudePluginNotice {
    /// The Claude Code plugin is not installed. The popup offers a
    /// `[prompt]` copy button that hands an LLM the migration recipe
    /// (clean up `~/.claude/settings.json` then run `/plugin install`).
    InstallRecommended,
    /// The plugin is installed AND the user still has legacy
    /// `tmux-agent-sidebar/hook.sh` entries in `~/.claude/settings.json`.
    /// Every hook fires twice in this state — once via the plugin, once
    /// via the manual setting. Takes precedence over `Stale` because it
    /// is an actively-broken state, not just a pending update.
    DuplicateHooks,
    /// The plugin is installed but its `plugin.json` version is older
    /// than the running binary, so the user needs to restart Claude
    /// Code to pick up the new bundled hooks.
    Stale { installed: String, current: String },
}

/// Click target for the `copy` label next to an agent in the notices popup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoticesCopyTarget {
    pub area: ratatui::layout::Rect,
    pub agent: String,
}
