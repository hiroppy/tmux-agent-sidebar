use std::time::Instant;

use crate::cli::plugin_state::ClaudePluginStatus;

/// Sub-state for the ⓘ notices popup, lifted out of [`AppState`] so its
/// seven related fields (button column, missing-hook groups, plugin
/// status, legacy hook flag, plugin notice, copy targets, copy feedback)
/// travel as a single unit.
#[derive(Debug, Clone, Default)]
pub struct NoticesState {
    /// Column of the ⓘ button in the secondary header, or `None` when the
    /// button is hidden. Used for click hit-testing.
    pub button_col: Option<u16>,
    /// Missing hooks grouped per agent, shown in the "Missing hooks"
    /// section of the popup.
    pub missing_hook_groups: Vec<NoticesMissingHookGroup>,
    /// Status of the `tmux-agent-sidebar` Claude Code plugin install
    /// (whether it is installed, and whether any tracked file in its
    /// cache differs from the copy embedded into this binary). Resolved
    /// once from `~/.claude/plugins/installed_plugins.json` and cached
    /// for the lifetime of the TUI process — restart the sidebar after
    /// a `/plugin install`, `/plugin uninstall`, or `/plugin update` to
    /// pick up the change. `claude_plugin_notice` and the missing-hooks
    /// Claude filter are derived from this field.
    pub claude_plugin_status: ClaudePluginStatus,
    /// Whether `~/.claude/settings.json` still contains residual
    /// `tmux-agent-sidebar/hook.sh` entries from the legacy manual
    /// setup. Resolved once at startup. When this is `true` AND the
    /// plugin is installed, every hook fires twice and the popup must
    /// keep nagging the user to clean up.
    pub claude_settings_has_residual_hooks: bool,
    /// Drives the `Plugin / claude` section in the notices popup. See
    /// [`ClaudePluginNotice`] for the full set of variants. Derived from
    /// `claude_plugin_status` in `refresh_notices`.
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
/// is installed, its cached hooks match the embedded snapshot, and the
/// user has no residual manual hook entries, no notice is set.
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
    /// The plugin is installed but at least one file tracked by
    /// `EMBEDDED_PLUGIN_FILES` differs between its cache and the
    /// snapshot embedded in the running binary. The user needs
    /// `/plugin update` so Claude Code re-reads the affected files.
    /// Comparing file content (rather than the manifest `version`
    /// string) means the notice only fires when an update actually
    /// changes fork behavior — the `hook.sh` wrapper already runs the
    /// latest binary on every invocation, so a bare version bump with
    /// no content changes is silent.
    Stale,
}

/// Click target for the `copy` label next to an agent in the notices popup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoticesCopyTarget {
    pub area: ratatui::layout::Rect,
    pub agent: String,
}
