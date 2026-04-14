use std::collections::HashMap;
use std::time::Instant;

use crate::activity::{ActivityEntry, TaskProgress};
use crate::tmux;
use crate::ui::colors::ColorTheme;
use crate::ui::icons::StatusIcons;

mod refresh;
mod tab;
#[cfg(test)]
pub(crate) use refresh::{TaskProgressDecision, classify_task_progress};

#[derive(Debug, Clone, PartialEq)]
pub enum Focus {
    Filter,
    Panes,
    ActivityLog,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StatusFilter {
    All,
    Running,
    Waiting,
    Idle,
    Error,
}

impl StatusFilter {
    pub const VARIANTS: [StatusFilter; 5] = [
        StatusFilter::All,
        StatusFilter::Running,
        StatusFilter::Waiting,
        StatusFilter::Idle,
        StatusFilter::Error,
    ];

    pub fn next(self) -> Self {
        let idx = StatusFilter::VARIANTS
            .iter()
            .position(|v| *v == self)
            .unwrap_or(0);
        StatusFilter::VARIANTS[(idx + 1) % StatusFilter::VARIANTS.len()]
    }

    pub fn prev(self) -> Self {
        let idx = StatusFilter::VARIANTS
            .iter()
            .position(|v| *v == self)
            .unwrap_or(0);
        StatusFilter::VARIANTS
            [(idx + StatusFilter::VARIANTS.len() - 1) % StatusFilter::VARIANTS.len()]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Running => "running",
            Self::Waiting => "waiting",
            Self::Idle => "idle",
            Self::Error => "error",
        }
    }

    /// Parse a tmux-option label into a `StatusFilter`. Unknown values
    /// fall back to `All`.
    pub fn from_label(s: &str) -> Self {
        match s {
            "running" => Self::Running,
            "waiting" => Self::Waiting,
            "idle" => Self::Idle,
            "error" => Self::Error,
            _ => Self::All,
        }
    }

    pub fn matches(self, status: &crate::tmux::PaneStatus) -> bool {
        match self {
            StatusFilter::All => true,
            StatusFilter::Running => *status == crate::tmux::PaneStatus::Running,
            StatusFilter::Waiting => *status == crate::tmux::PaneStatus::Waiting,
            StatusFilter::Idle => *status == crate::tmux::PaneStatus::Idle,
            StatusFilter::Error => *status == crate::tmux::PaneStatus::Error,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RepoFilter {
    All,
    Repo(String),
}

impl RepoFilter {
    pub fn as_str(&self) -> &str {
        match self {
            Self::All => "all",
            Self::Repo(name) => name.as_str(),
        }
    }

    /// Parse a tmux-option label into a `RepoFilter`. `""` and `"all"`
    /// map to `All`; any other value is stored as `Repo(name)`.
    pub fn from_label(s: &str) -> Self {
        match s {
            "all" | "" => Self::All,
            name => Self::Repo(name.to_string()),
        }
    }

    pub fn matches_group(&self, group_name: &str) -> bool {
        match self {
            Self::All => true,
            Self::Repo(name) => name == group_name,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BottomTab {
    Activity,
    GitStatus,
}

/// Per-pane runtime state that should vanish together with the pane.
#[derive(Debug, Clone, Default)]
pub struct PaneRuntimeState {
    pub ports: Vec<u16>,
    pub command: Option<String>,
    pub task_progress: Option<TaskProgress>,
    pub task_dismissed_total: Option<usize>,
    pub inactive_since: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct RowTarget {
    pub pane_id: String,
}

/// At-most-one popup state for the sidebar. The enum variant encodes
/// both which popup is open and its per-popup data, so the "only one
/// popup open at a time" invariant is checked by the type system.
#[derive(Debug, Clone, Default)]
pub enum PopupState {
    #[default]
    None,
    Repo {
        selected: usize,
        area: Option<ratatui::layout::Rect>,
    },
    Notices {
        area: Option<ratatui::layout::Rect>,
    },
}

impl PopupState {
    pub fn set_repo_area(&mut self, rect: Option<ratatui::layout::Rect>) {
        if let Self::Repo { area, .. } = self {
            *area = rect;
        }
    }

    pub fn set_notices_area(&mut self, rect: Option<ratatui::layout::Rect>) {
        if let Self::Notices { area } = self {
            *area = rect;
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    pub offset: usize,
    pub total_lines: usize,
    pub visible_height: usize,
}

impl ScrollState {
    pub fn scroll(&mut self, delta: isize) {
        let max = self.total_lines.saturating_sub(self.visible_height);
        let next = self.offset as isize + delta;
        self.offset = next.max(0).min(max as isize) as usize;
    }
}

/// State shared across all sidebar instances via tmux global variables.
/// Synced from tmux at startup and on pane focus change (SIGUSR1).
pub struct GlobalState {
    pub status_filter: StatusFilter,
    pub selected_pane_row: usize,
    pub repo_filter: RepoFilter,
    /// Last filter value successfully written to tmux.
    last_saved_filter: StatusFilter,
    /// Last cursor value successfully written to tmux.
    last_saved_cursor: usize,
    /// Last repo filter value successfully written to tmux.
    last_saved_repo_filter: RepoFilter,
}

impl Default for GlobalState {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalState {
    pub fn new() -> Self {
        Self {
            status_filter: StatusFilter::All,
            selected_pane_row: 0,
            repo_filter: RepoFilter::All,
            last_saved_filter: StatusFilter::All,
            last_saved_cursor: 0,
            last_saved_repo_filter: RepoFilter::All,
        }
    }

    /// Save filter to tmux global variable.
    /// Only updates `last_saved_filter` on success so that a failed write
    /// does not cause sync to overwrite the user's choice.
    pub fn save_filter(&mut self) {
        if tmux::run_tmux(&["set", "-g", "@sidebar_filter", self.status_filter.as_str()]).is_some()
        {
            self.last_saved_filter = self.status_filter;
        }
    }

    /// Save cursor position to tmux global variable.
    pub fn save_cursor(&mut self) {
        if tmux::run_tmux(&[
            "set",
            "-g",
            "@sidebar_cursor",
            &self.selected_pane_row.to_string(),
        ])
        .is_some()
        {
            self.last_saved_cursor = self.selected_pane_row;
        }
    }

    /// Save repo filter to tmux global variable.
    pub fn save_repo_filter(&mut self) {
        if tmux::run_tmux(&[
            "set",
            "-g",
            "@sidebar_repo_filter",
            self.repo_filter.as_str(),
        ])
        .is_some()
        {
            self.last_saved_repo_filter = self.repo_filter.clone();
        }
    }

    /// Load all global state from tmux variables.
    /// Called at startup and on SIGUSR1 (pane focus change).
    pub fn load_from_tmux(&mut self) {
        let opts = tmux::get_all_global_options();
        self.apply_all(&opts);
    }

    /// Apply all global options from tmux (filter, cursor, repo filter).
    pub fn apply_all(&mut self, opts: &HashMap<String, String>) {
        if let Some(filter_str) = opts.get("@sidebar_filter") {
            let tmux_filter = StatusFilter::from_label(filter_str);
            if tmux_filter != self.last_saved_filter {
                self.status_filter = tmux_filter;
                self.last_saved_filter = tmux_filter;
            }
        }
        if let Some(cursor_str) = opts.get("@sidebar_cursor")
            && let Ok(n) = cursor_str.parse::<usize>()
            && n != self.last_saved_cursor
        {
            self.selected_pane_row = n;
            self.last_saved_cursor = n;
        }
        if let Some(repo_str) = opts.get("@sidebar_repo_filter") {
            let tmux_repo = RepoFilter::from_label(repo_str);
            if tmux_repo != self.last_saved_repo_filter {
                self.repo_filter = tmux_repo.clone();
                self.last_saved_repo_filter = tmux_repo;
            }
        }
    }
}

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
    /// [`ClaudePluginNotice`] for the two states. Derived from
    /// `claude_plugin_installed_version` in `refresh_notices`.
    pub claude_plugin_notice: Option<ClaudePluginNotice>,
    /// Click regions for the `copy` label on each agent row in the popup.
    pub copy_targets: Vec<NoticesCopyTarget>,
    /// Agent name and timestamp of the most recent successful copy, shown
    /// as a transient `copied` label next to the popup title.
    pub copied_at: Option<(String, Instant)>,
}

pub struct AppState {
    pub now: u64,
    pub repo_groups: Vec<crate::group::RepoGroup>,
    pub sidebar_focused: bool,
    pub focus: Focus,
    pub spinner_frame: usize,
    pub pane_row_targets: Vec<RowTarget>,
    pub activity_entries: Vec<ActivityEntry>,
    pub activity_scroll: ScrollState,
    pub focused_pane_id: Option<String>,
    pub tmux_pane: String,
    pub activity_max_entries: usize,
    pub line_to_row: Vec<Option<usize>>,
    pub panes_scroll: ScrollState,
    pub theme: ColorTheme,
    pub icons: StatusIcons,
    pub bottom_tab: BottomTab,
    pub git: crate::git::GitData,
    pub git_scroll: ScrollState,
    pub pane_states: HashMap<String, PaneRuntimeState>,
    /// Agent pane IDs that have already been seen.
    pub seen_agent_panes: std::collections::HashSet<String>,
    /// Per-pane bottom tab preference.
    pub pane_tab_prefs: HashMap<String, BottomTab>,
    /// Previous focused pane ID, used to detect focus changes.
    pub prev_focused_pane_id: Option<String>,
    /// Last time a mouse click was processed on the filter bar (for debounce).
    pub last_filter_click: std::time::Instant,
    /// Current popup state. At most one popup is open at a time; the enum
    /// variant encodes both which popup is open and its per-popup data.
    pub popup: PopupState,
    /// All fields related to the ⓘ notices button and its popup — the button
    /// click region, cached hook/plugin diagnostics, per-agent copy targets,
    /// and the transient "copied" feedback label.
    pub notices: NoticesState,
    /// Pending OSC 52 clipboard payload. The main loop flushes this to
    /// stdout after the next frame so tmux (with `set-clipboard on`) can
    /// forward it to the upstream terminal's clipboard — covering the
    /// SSH case where `arboard` would only reach the remote machine.
    pub pending_osc52_copy: Option<String>,
    pub repo_button_col: Option<u16>,
    /// Update notice shown when a newer GitHub release is available.
    pub version_notice: Option<crate::version::UpdateNotice>,
    /// Shared state across sidebar instances, persisted to tmux global variables.
    pub global: GlobalState,
    /// Hyperlink overlays to be written after frame render (OSC 8).
    pub hyperlink_overlays: Vec<HyperlinkOverlay>,
    pub port_scan_initialized: bool,
    pub last_port_refresh: Instant,
    /// Height of the bottom panel in lines. Loaded once at startup from
    /// the `@sidebar_bottom_height` tmux option. A value of 0 hides the panel.
    pub bottom_panel_height: u16,
    /// Maps session_id → session name, refreshed periodically from
    /// `~/.claude/sessions/*.json` files.
    pub session_names: HashMap<String, String>,
    pub last_session_refresh: Instant,
}

/// Screen-positioned hyperlink overlay for OSC 8 terminal hyperlinks.
#[derive(Debug, Clone)]
pub struct HyperlinkOverlay {
    pub x: u16,
    pub y: u16,
    pub text: String,
    pub url: String,
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

impl AppState {
    pub fn new(tmux_pane: String) -> Self {
        Self {
            now: 0,
            repo_groups: vec![],
            sidebar_focused: false,
            focus: Focus::Panes,
            spinner_frame: 0,
            pane_row_targets: vec![],
            activity_entries: vec![],
            activity_scroll: ScrollState::default(),
            focused_pane_id: None,
            tmux_pane,
            activity_max_entries: 50,
            line_to_row: vec![],
            panes_scroll: ScrollState::default(),
            theme: ColorTheme::default(),
            icons: StatusIcons::default(),
            bottom_tab: BottomTab::Activity,
            git: crate::git::GitData::default(),
            git_scroll: ScrollState::default(),
            pane_states: HashMap::new(),
            seen_agent_panes: std::collections::HashSet::new(),
            pane_tab_prefs: HashMap::new(),
            prev_focused_pane_id: None,
            last_filter_click: std::time::Instant::now(),
            popup: PopupState::None,
            notices: NoticesState::default(),
            pending_osc52_copy: None,
            repo_button_col: None,
            version_notice: None,
            global: GlobalState::new(),
            hyperlink_overlays: vec![],
            port_scan_initialized: false,
            last_port_refresh: Instant::now(),
            bottom_panel_height: crate::ui::BOTTOM_PANEL_HEIGHT,
            session_names: HashMap::new(),
            last_session_refresh: Instant::now(),
        }
    }

    pub fn pane_state_mut(&mut self, pane_id: &str) -> &mut PaneRuntimeState {
        self.pane_states.entry(pane_id.to_string()).or_default()
    }

    pub fn pane_state(&self, pane_id: &str) -> Option<&PaneRuntimeState> {
        self.pane_states.get(pane_id)
    }

    pub fn pane_by_id(&self, pane_id: &str) -> Option<&crate::tmux::PaneInfo> {
        for group in &self.repo_groups {
            for (pane, _) in &group.panes {
                if pane.pane_id == pane_id {
                    return Some(pane);
                }
            }
        }
        None
    }

    pub fn selected_pane(&self) -> Option<&crate::tmux::PaneInfo> {
        let target = self.pane_row_targets.get(self.global.selected_pane_row)?;
        self.pane_by_id(&target.pane_id)
    }

    pub fn set_pane_ports(&mut self, pane_id: &str, ports: Vec<u16>) {
        self.pane_state_mut(pane_id).ports = ports;
    }

    pub fn pane_ports(&self, pane_id: &str) -> Option<&[u16]> {
        self.pane_state(pane_id).map(|s| s.ports.as_slice())
    }

    pub fn set_pane_command(&mut self, pane_id: &str, command: Option<String>) {
        self.pane_state_mut(pane_id).command = command;
    }

    pub fn pane_command(&self, pane_id: &str) -> Option<&str> {
        self.pane_state(pane_id).and_then(|s| s.command.as_deref())
    }

    pub fn set_pane_task_progress(&mut self, pane_id: &str, progress: Option<TaskProgress>) {
        self.pane_state_mut(pane_id).task_progress = progress;
    }

    pub fn pane_task_progress(&self, pane_id: &str) -> Option<&TaskProgress> {
        self.pane_state(pane_id)
            .and_then(|s| s.task_progress.as_ref())
    }

    pub fn set_pane_task_dismissed_total(&mut self, pane_id: &str, total: Option<usize>) {
        self.pane_state_mut(pane_id).task_dismissed_total = total;
    }

    pub fn pane_task_dismissed_total(&self, pane_id: &str) -> Option<usize> {
        self.pane_state(pane_id)
            .and_then(|s| s.task_dismissed_total)
    }

    pub fn set_pane_inactive_since(&mut self, pane_id: &str, since: Option<u64>) {
        self.pane_state_mut(pane_id).inactive_since = since;
    }

    pub fn pane_inactive_since(&self, pane_id: &str) -> Option<u64> {
        self.pane_state(pane_id).and_then(|s| s.inactive_since)
    }

    pub fn clear_pane_state(&mut self, pane_id: &str) {
        self.pane_states.remove(pane_id);
    }

    /// Resolve the notices popup inputs once.
    ///
    /// Every input is static for the sidebar's lifetime:
    /// `claude_plugin_installed_version` and
    /// `claude_settings_has_residual_hooks` are resolved at `main.rs`
    /// startup, and `settings.json` / `hooks.json` edits only take
    /// effect after a sidebar restart — matching the restart-required
    /// contract already documented for `/plugin install`. So this runs
    /// once from `main.rs` instead of being pinned to the per-tick
    /// refresh loop, and the ⓘ badge no longer depends on which pane
    /// happens to be focused.
    ///
    /// Both Claude and Codex are always evaluated so a user who closes
    /// their last agent pane still sees any outstanding hook setup
    /// warnings.
    pub fn refresh_notices(&mut self) {
        self.notices.claude_plugin_notice = compute_claude_plugin_notice(
            self.notices.claude_plugin_installed_version.as_deref(),
            crate::VERSION,
            self.notices.claude_settings_has_residual_hooks,
        );

        // Suppress Claude from the missing-hooks list whenever the
        // plugin is installed. Residual legacy entries are already
        // surfaced by the Plugin section's `DuplicateHooks` notice, so
        // re-adding Claude here would only duplicate the warning.
        let claude_plugin_present = self.notices.claude_plugin_installed_version.is_some();

        let hook_script = crate::cli::setup::resolve_hook_script().path;
        let force_missing = debug_forced_display();
        let load_config = |agent: &str| -> serde_json::Value {
            if force_missing {
                serde_json::Value::Null
            } else {
                crate::cli::setup::load_current_config(agent)
            }
        };
        self.notices.missing_hook_groups = compute_missing_hook_groups(
            claude_plugin_present,
            vec![
                crate::tmux::CLAUDE_AGENT.to_string(),
                crate::tmux::CODEX_AGENT.to_string(),
            ],
            &hook_script,
            load_config,
        );
    }

    pub fn is_repo_popup_open(&self) -> bool {
        matches!(self.popup, PopupState::Repo { .. })
    }

    pub fn is_notices_popup_open(&self) -> bool {
        matches!(self.popup, PopupState::Notices { .. })
    }

    pub fn repo_popup_selected(&self) -> usize {
        match &self.popup {
            PopupState::Repo { selected, .. } => *selected,
            _ => 0,
        }
    }

    pub fn set_repo_popup_selected(&mut self, n: usize) {
        if let PopupState::Repo { selected, .. } = &mut self.popup {
            *selected = n;
        }
    }

    pub fn repo_popup_area(&self) -> Option<ratatui::layout::Rect> {
        match &self.popup {
            PopupState::Repo { area, .. } => *area,
            _ => None,
        }
    }

    pub fn notices_popup_area(&self) -> Option<ratatui::layout::Rect> {
        match &self.popup {
            PopupState::Notices { area } => *area,
            _ => None,
        }
    }

    pub fn toggle_notices_popup(&mut self) {
        if self.is_notices_popup_open() {
            self.close_notices_popup();
        } else {
            self.popup = PopupState::Notices { area: None };
        }
    }

    pub fn close_notices_popup(&mut self) {
        self.popup = PopupState::None;
        self.notices.copy_targets.clear();
        self.notices.copied_at = None;
    }

    /// Return the agent name if the given (row, col) hits a `[copy]` label
    /// in the currently rendered notices popup. Pure lookup — no side effects.
    pub fn notices_copy_target_at(&self, row: u16, col: u16) -> Option<&str> {
        self.notices
            .copy_targets
            .iter()
            .find(|t| {
                row >= t.area.y
                    && row < t.area.y + t.area.height
                    && col >= t.area.x
                    && col < t.area.x + t.area.width
            })
            .map(|t| t.agent.as_str())
    }

    /// Copy the LLM setup prompt for the given agent (`claude` / `codex`)
    /// to every clipboard-reachable surface: `arboard` for the local OS
    /// clipboard, `tmux set-buffer` for the tmux paste buffer, and a
    /// queued OSC 52 escape (flushed by the main loop) for upstream
    /// terminals over SSH. Returns true only when at least one *verifiable*
    /// destination succeeded so the caller can decide whether to show the
    /// `[copied]` feedback.
    pub fn copy_notices_prompt(&mut self, agent: &str) -> bool {
        let Some(prompt) = crate::ui::notices::prompt_for_agent(agent) else {
            return false;
        };
        let clip_ok = arboard::Clipboard::new()
            .and_then(|mut c| c.set_text(prompt.clone()))
            .is_ok();
        let tmux_ok = std::process::Command::new("tmux")
            .args(["set-buffer", &prompt])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        // OSC 52 is queued regardless — it reaches the upstream terminal
        // even when the local sinks above fail (SSH case). But we do not
        // count it toward the feedback because there is no success signal.
        self.pending_osc52_copy = Some(prompt);
        self.record_notices_copy_result(agent, clip_ok || tmux_ok)
    }

    /// Stamp the `[copied]` feedback state. Pure separation from the I/O
    /// above so the success policy is unit-testable without touching the
    /// real clipboard or tmux.
    pub fn record_notices_copy_result(&mut self, agent: &str, success: bool) -> bool {
        if success {
            self.notices.copied_at = Some((agent.to_string(), Instant::now()));
        }
        success
    }

    pub fn prune_pane_states_to_current_panes(&mut self) {
        let mut active_ids = std::collections::HashSet::new();
        for group in &self.repo_groups {
            for (pane, _) in &group.panes {
                active_ids.insert(pane.pane_id.clone());
            }
        }
        self.pane_states
            .retain(|pane_id, _| active_ids.contains(pane_id));
    }

    pub fn rebuild_row_targets(&mut self) {
        // Reset stale repo filter if the repo no longer exists
        if let RepoFilter::Repo(ref name) = self.global.repo_filter
            && !self.repo_groups.iter().any(|g| g.name == *name)
        {
            self.global.repo_filter = RepoFilter::All;
        }

        self.pane_row_targets.clear();
        for group in &self.repo_groups {
            if !self.global.repo_filter.matches_group(&group.name) {
                continue;
            }
            for (pane, _) in &group.panes {
                if self.global.status_filter.matches(&pane.status) {
                    self.pane_row_targets.push(RowTarget {
                        pane_id: pane.pane_id.clone(),
                    });
                }
            }
        }
        if self.global.selected_pane_row >= self.pane_row_targets.len()
            && !self.pane_row_targets.is_empty()
        {
            self.global.selected_pane_row = self.pane_row_targets.len() - 1;
        }
    }

    pub fn find_focused_pane(&mut self) {
        // Query tmux directly for the active pane, not through `repo_groups`
        // which only contains agent panes. This allows activity/git info to
        // be displayed even when the focused pane has no agent running.
        // When the sidebar has focus, find_active_pane returns None — preserve
        // the previously focused pane so bottom panel data stays stable.
        if let Some((id, _)) = tmux::find_active_pane(&self.tmux_pane) {
            self.focused_pane_id = Some(id);
        }
    }

    /// Move agent selection. Returns true if moved, false if at boundary.
    pub fn move_pane_selection(&mut self, delta: isize) -> bool {
        if self.pane_row_targets.is_empty() {
            return false;
        }
        let len = self.pane_row_targets.len() as isize;
        let next = self.global.selected_pane_row as isize + delta;
        if next >= 0 && next < len {
            self.global.selected_pane_row = next as usize;
            true
        } else {
            false
        }
    }

    pub fn activate_selected_pane(&self) {
        if let Some(target) = self.pane_row_targets.get(self.global.selected_pane_row) {
            tmux::select_pane(&target.pane_id);
        }
    }

    pub fn next_bottom_tab(&mut self) {
        self.bottom_tab = match self.bottom_tab {
            BottomTab::Activity => BottomTab::GitStatus,
            BottomTab::GitStatus => BottomTab::Activity,
        };
    }

    /// Handle mouse click on the bottom panel tab header.
    /// Tab title layout: "╭ Activity │ Git ╮" — col is relative to the terminal.
    /// The block border starts at col 0, so the title text starts at col 1.
    /// " Activity " spans cols 1..11, "│" at col 11, " Git " spans cols 12..17.
    pub fn handle_bottom_tab_click(&mut self, col: u16) {
        // Offset by 1 for the left border character
        let x = col.saturating_sub(1) as usize;
        // " Activity " = 10 chars (0..10), "│" = 1 char (10), " Git " = 5 chars (11..16)
        if x < 10 {
            self.bottom_tab = BottomTab::Activity;
        } else if (11..16).contains(&x) {
            self.bottom_tab = BottomTab::GitStatus;
        }
    }

    pub fn scroll_bottom(&mut self, delta: isize) {
        match self.bottom_tab {
            BottomTab::Activity => self.activity_scroll.scroll(delta),
            BottomTab::GitStatus => self.git_scroll.scroll(delta),
        }
    }

    /// Handle mouse scroll event, routing to agents or bottom panel based on Y position.
    pub fn handle_mouse_scroll(
        &mut self,
        row: u16,
        term_height: u16,
        bottom_panel_height: u16,
        delta: isize,
    ) {
        let bottom_start = term_height.saturating_sub(bottom_panel_height);
        if row >= bottom_start {
            self.scroll_bottom(delta);
        } else {
            self.panes_scroll.scroll(delta);
        }
    }

    /// Handle mouse click on the filter bar (row 0).
    /// Determines which filter was clicked based on x coordinate.
    /// Debounces rapid clicks to ignore phantom mouse events from tmux
    /// pane resize/layout changes.
    pub fn handle_filter_click(&mut self, col: u16) {
        const DEBOUNCE_MS: u128 = 150;
        let now = std::time::Instant::now();
        if now.duration_since(self.last_filter_click).as_millis() < DEBOUNCE_MS {
            return;
        }
        self.last_filter_click = now;

        let (all, running, waiting, idle, error) = self.status_counts();
        // Layout: " ∑N  ●N  ◐N  ○N  ✕N"
        // Each filter item renders as `icon(1) + count`, so the clickable
        // width is `1 + digits(count)`.
        let mut x = 1usize; // leading space
        let items: Vec<(StatusFilter, usize)> = vec![
            (StatusFilter::All, 1 + format!("{all}").len()),
            (StatusFilter::Running, 1 + format!("{running}").len()),
            (StatusFilter::Waiting, 1 + format!("{waiting}").len()),
            (StatusFilter::Idle, 1 + format!("{idle}").len()),
            (StatusFilter::Error, 1 + format!("{error}").len()),
        ];
        let col = col as usize;
        for (i, (filter, width)) in items.iter().enumerate() {
            if i > 0 {
                x += 2; // "  " separator
            }
            if col >= x && col < x + width {
                self.global.status_filter = *filter;
                self.global.save_filter();
                self.rebuild_row_targets();
                return;
            }
            x += width;
        }
    }

    /// Handle mouse click on the secondary header row (row 1).
    /// The repo filter button lives on the far right of this row.
    pub fn handle_secondary_header_click(&mut self, col: u16) {
        if self
            .notices
            .button_col
            .is_some_and(|notices_col| col == notices_col)
        {
            self.toggle_notices_popup();
            return;
        }
        if self
            .repo_button_col
            .is_some_and(|repo_button_col| col >= repo_button_col)
        {
            self.toggle_repo_popup();
        }
    }

    /// Handle mouse click in agents panel. Maps screen row to agent row
    /// via line_to_row (adjusted for scroll offset) and activates that pane.
    /// Row 0 is the fixed filter bar, row 1+ maps to the scrollable agent list.
    pub fn handle_mouse_click(&mut self, row: u16, col: u16) {
        // Handle popup interactions first
        if self.is_notices_popup_open() {
            if let Some(popup_area) = self.notices_popup_area()
                && row >= popup_area.y
                && row < popup_area.y + popup_area.height
                && col >= popup_area.x
                && col < popup_area.x + popup_area.width
            {
                if let Some(agent) = self.notices_copy_target_at(row, col).map(str::to_string) {
                    self.copy_notices_prompt(&agent);
                }
                return;
            }
            self.close_notices_popup();
            return;
        }
        if self.is_repo_popup_open() {
            if let Some(popup_area) = self.repo_popup_area()
                && row >= popup_area.y
                && row < popup_area.y + popup_area.height
                && col >= popup_area.x
                && col < popup_area.x + popup_area.width
            {
                // Click inside popup — select item (subtract 1 for top border)
                let item_index = (row - popup_area.y).saturating_sub(1) as usize;
                let repos = self.repo_names();
                if item_index < repos.len() {
                    self.set_repo_popup_selected(item_index);
                    self.confirm_repo_popup();
                }
                return;
            }
            // Click outside popup — close it
            self.close_repo_popup();
            return;
        }

        if row == 0 {
            self.handle_filter_click(col);
            return;
        }
        if row == 1 {
            self.handle_secondary_header_click(col);
            return;
        }
        let line_index = (row as usize - 2) + self.panes_scroll.offset;
        if let Some(Some(agent_row)) = self.line_to_row.get(line_index) {
            self.global.selected_pane_row = *agent_row;
            self.global.save_cursor();
            self.activate_selected_pane();
        }
    }

    /// Count agents per status across all repo groups.
    pub fn status_counts(&self) -> (usize, usize, usize, usize, usize) {
        let (mut running, mut waiting, mut idle, mut error) = (0, 0, 0, 0);
        for group in &self.repo_groups {
            if !self.global.repo_filter.matches_group(&group.name) {
                continue;
            }
            for (pane, _) in &group.panes {
                match pane.status {
                    crate::tmux::PaneStatus::Running => running += 1,
                    crate::tmux::PaneStatus::Waiting => waiting += 1,
                    crate::tmux::PaneStatus::Idle => idle += 1,
                    crate::tmux::PaneStatus::Error => error += 1,
                    crate::tmux::PaneStatus::Unknown => {}
                }
            }
        }
        let all = running + waiting + idle + error;
        (all, running, waiting, idle, error)
    }

    pub fn apply_git_data(&mut self, data: crate::git::GitData) {
        self.git = data;
    }

    /// Return list of repo names for the popup: ["All", repo1, repo2, ...]
    pub fn repo_names(&self) -> Vec<String> {
        let mut names = vec!["All".to_string()];
        for group in &self.repo_groups {
            names.push(group.name.clone());
        }
        names
    }

    pub fn toggle_repo_popup(&mut self) {
        if self.is_repo_popup_open() {
            self.close_repo_popup();
            return;
        }
        // Set selected to current filter position
        let names = self.repo_names();
        let selected = match &self.global.repo_filter {
            RepoFilter::All => 0,
            RepoFilter::Repo(name) => names.iter().position(|n| n == name).unwrap_or(0),
        };
        self.popup = PopupState::Repo {
            selected,
            area: None,
        };
    }

    pub fn confirm_repo_popup(&mut self) {
        let selected = self.repo_popup_selected();
        let names = self.repo_names();
        if let Some(name) = names.get(selected) {
            self.global.repo_filter = if selected == 0 {
                RepoFilter::All
            } else {
                RepoFilter::Repo(name.clone())
            };
        }
        self.popup = PopupState::None;
        self.global.save_repo_filter();
        self.rebuild_row_targets();
    }

    pub fn close_repo_popup(&mut self) {
        self.popup = PopupState::None;
    }
}

/// Compute the per-agent missing-hook list shown in the notices popup.
///
/// `claude_plugin_present` gates Claude visibility: when the plugin is
/// installed, it owns the hook wiring so Claude is filtered out (the
/// `Plugin / claude` section reports stale-version state instead). When
/// the plugin is **not** installed, Claude must surface concrete
/// missing-hook diagnostics for users still on the manual
/// `~/.claude/settings.json` path. Codex is unaffected — Codex CLI has
/// no plugin mechanism upstream.
///
/// Pure function: takes the agent list and a config loader as inputs so
/// tests do not need to manipulate `/tmp` files or `~/.claude/`.
fn compute_missing_hook_groups(
    claude_plugin_present: bool,
    agents: Vec<String>,
    hook_script: &str,
    load_config: impl Fn(&str) -> serde_json::Value,
) -> Vec<NoticesMissingHookGroup> {
    agents
        .into_iter()
        .filter(|agent| !(claude_plugin_present && agent == "claude"))
        .filter_map(|agent| {
            let config = load_config(&agent);
            let hooks = crate::cli::setup::missing_hooks(&agent, &config, hook_script);
            if hooks.is_empty() {
                None
            } else {
                Some(NoticesMissingHookGroup { agent, hooks })
            }
        })
        .collect()
}

/// Build the `Plugin / claude` notice based on the recorded plugin
/// version and whether residual manual hook entries remain in
/// `~/.claude/settings.json`. Priority order:
///
/// - No plugin install → `InstallRecommended` (the migration prompt
///   handles legacy cleanup as part of the same step, so `has_residual`
///   does not matter here).
/// - Plugin installed + residual entries → `DuplicateHooks`. Takes
///   precedence over `Stale` because hooks are firing twice right now
///   and the cleanup is more urgent than a pending version bump.
/// - Plugin installed, no residual, version mismatch → `Stale`.
/// - Plugin installed, no residual, version match → no notice.
fn compute_claude_plugin_notice(
    installed_version: Option<&str>,
    current_version: &str,
    has_residual_hooks: bool,
) -> Option<ClaudePluginNotice> {
    match installed_version {
        None => Some(ClaudePluginNotice::InstallRecommended),
        Some(_) if has_residual_hooks => Some(ClaudePluginNotice::DuplicateHooks),
        Some(v) if v == current_version => None,
        Some(v) => Some(ClaudePluginNotice::Stale {
            installed: v.to_string(),
            current: current_version.to_string(),
        }),
    }
}

pub(crate) fn debug_forced_display() -> bool {
    cfg!(feature = "debug")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity::{TaskProgress, TaskStatus};
    use crate::group::{PaneGitInfo, RepoGroup};
    use crate::tmux::{AgentType, PaneInfo, PaneStatus, PermissionMode};
    use std::fs;

    /// Reset filter click debounce so the next `handle_filter_click` is not ignored.
    fn reset_filter_debounce(state: &mut AppState) {
        state.last_filter_click = std::time::Instant::now() - std::time::Duration::from_millis(200);
    }

    // ─── compute_missing_hook_groups: claude_plugin_present gating ───

    /// Return a config loader that always reports an empty config — i.e.
    /// every hook the adapter expects is reported as missing. This keeps
    /// these tests focused on the agent filtering logic instead of on
    /// the `missing_hooks` algorithm itself.
    fn empty_config_loader() -> impl Fn(&str) -> serde_json::Value {
        |_agent: &str| serde_json::Value::Null
    }

    #[test]
    fn missing_hook_groups_includes_claude_when_plugin_not_installed() {
        // Manual `~/.claude/settings.json` path — Claude must surface
        // concrete missing-hook diagnostics so the user knows what to
        // wire up. The Plugin section's InstallRecommended notice is a
        // companion, not a substitute.
        let groups = compute_missing_hook_groups(
            /* claude_plugin_present */ false,
            vec!["claude".to_string()],
            "/fake/hook.sh",
            empty_config_loader(),
        );
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].agent, "claude");
        assert!(
            !groups[0].hooks.is_empty(),
            "claude should report missing hooks against an empty config"
        );
    }

    #[test]
    fn missing_hook_groups_skips_claude_when_plugin_installed() {
        // The plugin owns the hook wiring once installed, so Claude
        // must NOT appear under Missing hooks (the Plugin section
        // reports stale-version state separately).
        let groups = compute_missing_hook_groups(
            /* claude_plugin_present */ true,
            vec!["claude".to_string()],
            "/fake/hook.sh",
            empty_config_loader(),
        );
        assert!(
            groups.is_empty(),
            "claude must be filtered out when the plugin is detected, got {:?}",
            groups
        );
    }

    #[test]
    fn missing_hook_groups_keeps_codex_regardless_of_plugin() {
        let agents = vec!["codex".to_string()];

        let without = compute_missing_hook_groups(
            false,
            agents.clone(),
            "/fake/hook.sh",
            empty_config_loader(),
        );
        let with =
            compute_missing_hook_groups(true, agents, "/fake/hook.sh", empty_config_loader());
        assert_eq!(without, with);
        assert_eq!(without.len(), 1);
        assert_eq!(without[0].agent, "codex");
    }

    #[test]
    fn missing_hook_groups_drops_only_claude_when_both_agents_present_and_plugin_installed() {
        // Forced-debug rendering passes both agents; verify the filter
        // hits Claude alone without affecting the Codex row.
        let groups = compute_missing_hook_groups(
            true,
            vec!["claude".to_string(), "codex".to_string()],
            "/fake/hook.sh",
            empty_config_loader(),
        );
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].agent, "codex");
    }

    // ─── compute_claude_plugin_notice ────────────────────────────────

    #[test]
    fn plugin_notice_install_recommended_when_version_missing() {
        // No version recorded → the user has not run `/plugin install`
        // yet, so the popup should encourage them to. Residual hooks do
        // not change this — the migration prompt cleans them up too.
        assert_eq!(
            compute_claude_plugin_notice(None, "0.5.0", false),
            Some(ClaudePluginNotice::InstallRecommended)
        );
        assert_eq!(
            compute_claude_plugin_notice(None, "0.5.0", true),
            Some(ClaudePluginNotice::InstallRecommended)
        );
    }

    #[test]
    fn plugin_notice_none_when_versions_match_and_no_residual() {
        assert_eq!(
            compute_claude_plugin_notice(Some("0.5.0"), "0.5.0", false),
            None
        );
    }

    #[test]
    fn plugin_notice_stale_when_versions_differ_and_no_residual() {
        assert_eq!(
            compute_claude_plugin_notice(Some("0.4.3"), "0.5.0", false),
            Some(ClaudePluginNotice::Stale {
                installed: "0.4.3".into(),
                current: "0.5.0".into(),
            })
        );
    }

    #[test]
    fn plugin_notice_duplicate_hooks_when_residual_overrides_stale() {
        // Plugin is installed AND legacy entries are still in
        // settings.json. Hooks fire twice. The DuplicateHooks notice
        // takes precedence over Stale even when the version is also
        // out of date — cleanup is the more urgent action.
        assert_eq!(
            compute_claude_plugin_notice(Some("0.4.3"), "0.5.0", true),
            Some(ClaudePluginNotice::DuplicateHooks)
        );
        assert_eq!(
            compute_claude_plugin_notice(Some("0.5.0"), "0.5.0", true),
            Some(ClaudePluginNotice::DuplicateHooks)
        );
    }

    // ─── copy feedback policy ────────────────────────────────────────

    #[test]
    fn record_notices_copy_result_success_sets_copied_feedback() {
        let mut state = AppState::new(String::new());
        assert!(state.record_notices_copy_result("claude", true));
        let entry = state
            .notices
            .copied_at
            .as_ref()
            .expect("success path must set notices_copied_at");
        assert_eq!(entry.0, "claude");
    }

    #[test]
    fn record_notices_copy_result_failure_does_not_set_copied_feedback() {
        let mut state = AppState::new(String::new());
        // Pre-populate to assert the failure path does not overwrite it.
        state.notices.copied_at = None;
        assert!(!state.record_notices_copy_result("claude", false));
        assert!(
            state.notices.copied_at.is_none(),
            "`[copied]` must not flash when every clipboard sink failed"
        );
    }

    #[test]
    fn record_notices_copy_result_failure_preserves_previous_feedback() {
        let mut state = AppState::new(String::new());
        let earlier = (
            "codex".to_string(),
            std::time::Instant::now() - std::time::Duration::from_millis(10),
        );
        state.notices.copied_at = Some(earlier.clone());
        // A later copy that fails should not clobber an earlier success,
        // but more importantly it must not fabricate a success for itself.
        assert!(!state.record_notices_copy_result("claude", false));
        let still = state
            .notices
            .copied_at
            .as_ref()
            .expect("prior success should survive a subsequent failure");
        assert_eq!(still.0, earlier.0);
    }

    #[test]
    fn copy_notices_prompt_short_circuits_for_unknown_agent() {
        // `gemini` has no prompt definition, so the function must return
        // early with `false` and leave `notices_copied_at` untouched —
        // without touching the real clipboard or tmux at all.
        let mut state = AppState::new(String::new());
        state.notices.copied_at = None;
        assert!(!state.copy_notices_prompt("gemini"));
        assert!(state.notices.copied_at.is_none());
        assert!(
            state.pending_osc52_copy.is_none(),
            "unknown agents must not queue an OSC 52 payload"
        );
    }

    // ─── notices_copy_target_at hit detection ───────────────────────

    fn copy_target_fixture() -> AppState {
        let mut state = AppState::new(String::new());
        state.notices.copy_targets = vec![
            NoticesCopyTarget {
                area: ratatui::layout::Rect::new(10, 5, 8, 1),
                agent: "claude".into(),
            },
            NoticesCopyTarget {
                area: ratatui::layout::Rect::new(10, 7, 8, 1),
                agent: "codex".into(),
            },
        ];
        state
    }

    #[test]
    fn notices_copy_target_at_finds_claude_row() {
        let state = copy_target_fixture();
        assert_eq!(state.notices_copy_target_at(5, 10), Some("claude"));
        assert_eq!(state.notices_copy_target_at(5, 17), Some("claude"));
    }

    #[test]
    fn notices_copy_target_at_finds_codex_row() {
        let state = copy_target_fixture();
        assert_eq!(state.notices_copy_target_at(7, 12), Some("codex"));
    }

    #[test]
    fn notices_copy_target_at_misses_outside_target_bounds() {
        let state = copy_target_fixture();
        // Same row but to the left of the slot
        assert_eq!(state.notices_copy_target_at(5, 9), None);
        // Same row but to the right of the slot
        assert_eq!(state.notices_copy_target_at(5, 18), None);
        // Gap row between the two targets
        assert_eq!(state.notices_copy_target_at(6, 12), None);
    }

    #[test]
    fn notices_copy_target_at_returns_none_when_no_targets_tracked() {
        let state = AppState::new(String::new());
        assert_eq!(state.notices_copy_target_at(5, 10), None);
    }

    fn test_pane(id: &str) -> PaneInfo {
        PaneInfo {
            pane_id: id.into(),
            pane_active: false,
            status: PaneStatus::Running,
            attention: false,
            agent: AgentType::Claude,
            path: "/tmp".into(),
            current_command: String::new(),
            prompt: String::new(),
            prompt_is_response: false,
            started_at: None,
            wait_reason: String::new(),
            permission_mode: PermissionMode::Default,
            subagents: vec![],
            pane_pid: None,
            worktree_name: String::new(),
            worktree_branch: String::new(),
            session_id: None,
            session_name: String::new(),
        }
    }

    fn write_activity_log(pane_id: &str, contents: &str) -> String {
        let path = crate::activity::log_file_path(pane_id);
        fs::write(&path, contents).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn rebuild_row_targets_from_repo_groups() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![
            RepoGroup {
                name: "dotfiles".into(),
                has_focus: true,
                panes: vec![
                    (test_pane("%1"), PaneGitInfo::default()),
                    (test_pane("%2"), PaneGitInfo::default()),
                ],
            },
            RepoGroup {
                name: "app".into(),
                has_focus: false,
                panes: vec![(test_pane("%3"), PaneGitInfo::default())],
            },
        ];
        state.rebuild_row_targets();

        assert_eq!(state.pane_row_targets.len(), 3);
        assert_eq!(state.pane_row_targets[0].pane_id, "%1");
        assert_eq!(state.pane_row_targets[1].pane_id, "%2");
        assert_eq!(state.pane_row_targets[2].pane_id, "%3");
    }

    #[test]
    fn selection_crosses_repo_groups() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![
            RepoGroup {
                name: "dotfiles".into(),
                has_focus: true,
                panes: vec![(test_pane("%1"), PaneGitInfo::default())],
            },
            RepoGroup {
                name: "app".into(),
                has_focus: false,
                panes: vec![(test_pane("%5"), PaneGitInfo::default())],
            },
        ];
        state.rebuild_row_targets();

        // Start at first group
        assert_eq!(state.global.selected_pane_row, 0);
        assert_eq!(state.pane_row_targets[0].pane_id, "%1");

        // Move to second group
        assert!(state.move_pane_selection(1));
        assert_eq!(state.global.selected_pane_row, 1);
        assert_eq!(state.pane_row_targets[1].pane_id, "%5");
    }

    #[test]
    fn task_progress_hides_when_all_completed() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%100".to_string();

        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(test_pane("%100"), PaneGitInfo::default())],
        }];

        let log_path = crate::activity::log_file_path(&pane_id);
        fs::write(
            &log_path,
            "10:00|TaskCreate|#1 A\n10:01|TaskCreate|#2 B\n10:02|TaskUpdate|completed #1\n10:03|TaskUpdate|completed #2\n",
        ).unwrap();

        state.refresh_task_progress();

        // All completed → hidden immediately
        assert!(state.pane_task_progress(&pane_id).is_none());
        // Dismissed count should be recorded
        assert_eq!(state.pane_task_dismissed_total(&pane_id), Some(2));

        // Calling refresh again should still be hidden (no flicker)
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_none());

        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn task_progress_reshows_when_new_tasks_added() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%101".to_string();

        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(test_pane("%101"), PaneGitInfo::default())],
        }];

        // First: 1 task, completed → dismissed
        let log_path = crate::activity::log_file_path(&pane_id);
        fs::write(
            &log_path,
            "10:00|TaskCreate|#1 A\n10:01|TaskUpdate|completed #1\n",
        )
        .unwrap();
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_none());

        // Now add a new in-progress task → should re-show
        fs::write(
            &log_path,
            "10:00|TaskCreate|#1 A\n10:01|TaskUpdate|completed #1\n10:02|TaskCreate|#2 B\n10:03|TaskUpdate|in_progress #2\n",
        ).unwrap();
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_some());

        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn classify_task_progress_empty_clears() {
        let progress = TaskProgress { tasks: vec![] };
        assert_eq!(
            classify_task_progress(&progress, None),
            TaskProgressDecision::Clear
        );
    }

    #[test]
    fn classify_task_progress_in_progress_shows() {
        let progress = TaskProgress {
            tasks: vec![
                ("A".into(), TaskStatus::Completed),
                ("B".into(), TaskStatus::InProgress),
            ],
        };
        assert_eq!(
            classify_task_progress(&progress, None),
            TaskProgressDecision::Show
        );
    }

    #[test]
    fn classify_task_progress_completed_dismisses_once() {
        let progress = TaskProgress {
            tasks: vec![
                ("A".into(), TaskStatus::Completed),
                ("B".into(), TaskStatus::Completed),
            ],
        };
        assert_eq!(
            classify_task_progress(&progress, None),
            TaskProgressDecision::Dismiss { total: 2 }
        );
        assert_eq!(
            classify_task_progress(&progress, Some(2)),
            TaskProgressDecision::Skip
        );
    }

    #[test]
    fn classify_task_progress_completed_with_different_dismissal_dismisses_again() {
        let progress = TaskProgress {
            tasks: vec![
                ("A".into(), TaskStatus::Completed),
                ("B".into(), TaskStatus::Completed),
            ],
        };
        assert_eq!(
            classify_task_progress(&progress, Some(1)),
            TaskProgressDecision::Dismiss { total: 2 }
        );
    }

    #[test]
    fn refresh_now_updates_current_time() {
        let mut state = AppState::new("%99".into());
        state.refresh_now();
        assert!(state.now > 0);
    }

    #[test]
    fn refresh_activity_log_reads_focused_pane() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%201";
        let log_path = crate::activity::log_file_path(pane_id);
        fs::write(&log_path, "10:00|Read|old\n10:01|Edit|new\n").unwrap();
        state.focused_pane_id = Some(pane_id.into());
        state.activity_max_entries = 50;

        state.refresh_activity_log();

        assert_eq!(state.activity_entries.len(), 2);
        assert_eq!(state.activity_entries[0].tool, "Edit");
        assert_eq!(state.activity_entries[0].label, "new");
        assert_eq!(state.activity_entries[1].tool, "Read");

        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn refresh_activity_log_clears_without_focus() {
        let mut state = AppState::new("%99".into());
        state.activity_entries = vec![crate::activity::ActivityEntry {
            timestamp: "10:00".into(),
            tool: "Read".into(),
            label: "keep?".into(),
        }];

        state.focused_pane_id = None;
        state.refresh_activity_log();

        assert!(state.activity_entries.is_empty());
    }

    #[test]
    fn refresh_task_progress_clears_empty_logs_and_dismissal() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%202".to_string();
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(test_pane(&pane_id), PaneGitInfo::default())],
        }];
        state.set_pane_task_progress(
            &pane_id,
            Some(TaskProgress {
                tasks: vec![("stale".into(), TaskStatus::InProgress)],
            }),
        );
        state.set_pane_task_dismissed_total(&pane_id, Some(1));

        state.refresh_task_progress();

        assert!(state.pane_task_progress(&pane_id).is_none());
        assert_eq!(state.pane_task_dismissed_total(&pane_id), None);
    }

    #[test]
    fn refresh_task_progress_shows_in_progress_and_clears_dismissal() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%203".to_string();
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(test_pane(&pane_id), PaneGitInfo::default())],
        }];
        state.set_pane_task_dismissed_total(&pane_id, Some(1));
        let log_path = write_activity_log(
            &pane_id,
            "10:00|TaskCreate|#1 A\n10:01|TaskUpdate|in_progress #1\n",
        );

        state.refresh_task_progress();

        assert_eq!(state.pane_task_dismissed_total(&pane_id), None);
        assert_eq!(
            state.pane_task_progress(&pane_id).map(|p| p.total()),
            Some(1)
        );
        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn refresh_task_progress_records_completed_dismissal() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%204".to_string();
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(test_pane(&pane_id), PaneGitInfo::default())],
        }];
        let log_path = write_activity_log(
            &pane_id,
            "10:00|TaskCreate|#1 A\n10:01|TaskUpdate|completed #1\n",
        );

        state.refresh_task_progress();

        assert!(state.pane_task_progress(&pane_id).is_none());
        assert_eq!(state.pane_task_dismissed_total(&pane_id), Some(1));
        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn refresh_task_progress_skips_already_dismissed_completed_tasks() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%205".to_string();
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(test_pane(&pane_id), PaneGitInfo::default())],
        }];
        let log_path = write_activity_log(
            &pane_id,
            "10:00|TaskCreate|#1 A\n10:01|TaskUpdate|completed #1\n",
        );

        state.refresh_task_progress();
        assert_eq!(state.pane_task_dismissed_total(&pane_id), Some(1));
        assert!(state.pane_task_progress(&pane_id).is_none());

        state.refresh_task_progress();
        assert_eq!(state.pane_task_dismissed_total(&pane_id), Some(1));
        assert!(state.pane_task_progress(&pane_id).is_none());
        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn refresh_task_progress_drops_dismissals_for_inactive_panes() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%206".to_string();
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(test_pane(&pane_id), PaneGitInfo::default())],
        }];
        let log_path = write_activity_log(
            &pane_id,
            "10:00|TaskCreate|#1 A\n10:01|TaskUpdate|completed #1\n",
        );
        state.refresh_task_progress();
        assert_eq!(state.pane_task_dismissed_total(&pane_id), Some(1));

        // Pane removed — both dismissed and inactive_since should be cleaned up
        state.repo_groups.clear();
        state.set_pane_inactive_since(&pane_id, Some(100));
        state.refresh_task_progress();

        assert!(state.pane_state(&pane_id).is_none());
        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn pane_runtime_state_accessors_round_trip() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%213";

        state.set_pane_ports(pane_id, vec![3000, 5173]);
        state.set_pane_command(pane_id, Some("npm run dev".into()));
        state.set_pane_task_progress(
            pane_id,
            Some(TaskProgress {
                tasks: vec![("A".into(), TaskStatus::InProgress)],
            }),
        );
        state.set_pane_task_dismissed_total(pane_id, Some(4));
        state.set_pane_inactive_since(pane_id, Some(123));

        assert_eq!(state.pane_ports(pane_id), Some(&[3000, 5173][..]));
        assert_eq!(state.pane_command(pane_id), Some("npm run dev"));
        assert_eq!(
            state.pane_task_progress(pane_id).map(|p| p.total()),
            Some(1)
        );
        assert_eq!(state.pane_task_dismissed_total(pane_id), Some(4));
        assert_eq!(state.pane_inactive_since(pane_id), Some(123));

        state.clear_pane_state(pane_id);
        assert!(state.pane_state(pane_id).is_none());
    }

    #[test]
    fn prune_pane_states_to_current_panes_drops_stale_entries() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(test_pane("%1"), PaneGitInfo::default())],
        }];
        state.set_pane_ports("%1", vec![3000]);
        state.set_pane_command("%1", Some("npm run dev".into()));
        state.set_pane_ports("%2", vec![5173]);
        state.set_pane_task_dismissed_total("%2", Some(2));

        state.prune_pane_states_to_current_panes();

        assert_eq!(state.pane_ports("%1"), Some(&[3000][..]));
        assert_eq!(state.pane_command("%1"), Some("npm run dev"));
        assert!(state.pane_state("%2").is_none());
    }

    #[test]
    fn refresh_task_progress_dismisses_incomplete_tasks_when_agent_idle() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%207".to_string();
        let mut pane = test_pane(&pane_id);
        pane.status = PaneStatus::Idle;
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        // 5 out of 6 tasks completed — agent is idle so it won't update further
        let log_path = write_activity_log(
            &pane_id,
            "10:00|TaskCreate|#1 A\n10:01|TaskCreate|#2 B\n10:02|TaskCreate|#3 C\n10:03|TaskCreate|#4 D\n10:04|TaskCreate|#5 E\n10:05|TaskCreate|#6 F\n10:06|TaskUpdate|completed #1\n10:07|TaskUpdate|completed #2\n10:08|TaskUpdate|completed #3\n10:09|TaskUpdate|completed #4\n10:10|TaskUpdate|completed #5\n",
        );

        // First refresh: grace period starts, tasks still shown (not dismissed yet)
        state.now = 100;
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_some());
        assert!(state.pane_inactive_since(&pane_id).is_some());

        // After grace period (3s): should be dismissed
        state.now = 104;
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_none());
        assert_eq!(state.pane_task_dismissed_total(&pane_id), Some(6));
        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn refresh_task_progress_shows_incomplete_tasks_when_agent_running() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%208".to_string();
        // test_pane defaults to PaneStatus::Running
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(test_pane(&pane_id), PaneGitInfo::default())],
        }];
        let log_path = write_activity_log(
            &pane_id,
            "10:00|TaskCreate|#1 A\n10:01|TaskCreate|#2 B\n10:02|TaskUpdate|completed #1\n10:03|TaskUpdate|in_progress #2\n",
        );

        state.refresh_task_progress();

        // Agent is running, so incomplete tasks should still be shown
        assert!(state.pane_task_progress(&pane_id).is_some());
        assert_eq!(
            state.pane_task_progress(&pane_id).map(|p| p.total()),
            Some(2)
        );
        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn refresh_task_progress_dismisses_incomplete_tasks_when_agent_error() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%209".to_string();
        let mut pane = test_pane(&pane_id);
        pane.status = PaneStatus::Error;
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        let log_path = write_activity_log(
            &pane_id,
            "10:00|TaskCreate|#1 A\n10:01|TaskUpdate|in_progress #1\n",
        );

        // First refresh: grace period starts, tasks still shown
        state.now = 100;
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_some());

        // After grace period: agent errored out — dismiss incomplete tasks
        state.now = 104;
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_none());
        assert_eq!(state.pane_task_dismissed_total(&pane_id), Some(1));
        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn refresh_task_progress_debounce_resets_when_agent_resumes() {
        // Simulates brief idle flicker: agent goes idle then returns to running
        // before the grace period expires — tasks should remain visible.
        let mut state = AppState::new("%99".into());
        let pane_id = "%210".to_string();
        let mut pane = test_pane(&pane_id);
        pane.status = PaneStatus::Idle;
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        let log_path = write_activity_log(
            &pane_id,
            "10:00|TaskCreate|#1 A\n10:01|TaskCreate|#2 B\n10:02|TaskUpdate|completed #1\n",
        );

        // Agent is idle — grace timer starts, tasks still shown
        state.now = 100;
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_some());
        assert!(state.pane_inactive_since(&pane_id).is_some());

        // Agent returns to running before grace expires — timer resets
        let mut pane = test_pane(&pane_id);
        pane.status = PaneStatus::Running;
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        state.now = 102;
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_some());
        assert!(state.pane_inactive_since(&pane_id).is_none());

        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn refresh_task_progress_debounce_exact_boundary() {
        // Grace period is 3 seconds. At exactly 3s the condition is >=,
        // so it should dismiss.
        let mut state = AppState::new("%99".into());
        let pane_id = "%211".to_string();
        let mut pane = test_pane(&pane_id);
        pane.status = PaneStatus::Idle;
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        let log_path = write_activity_log(
            &pane_id,
            "10:00|TaskCreate|#1 A\n10:01|TaskUpdate|in_progress #1\n",
        );

        // t=100: grace timer starts
        state.now = 100;
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_some());

        // t=102 (2s elapsed): still within grace period — tasks shown
        state.now = 102;
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_some());

        // t=103 (exactly 3s): grace expired (>= 3) — dismissed
        state.now = 103;
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_none());
        assert_eq!(state.pane_task_dismissed_total(&pane_id), Some(1));

        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn refresh_task_progress_waiting_does_not_start_debounce() {
        // Waiting is an active state — inactive timer should not be set.
        let mut state = AppState::new("%99".into());
        let pane_id = "%212".to_string();
        let mut pane = test_pane(&pane_id);
        pane.status = PaneStatus::Waiting;
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        let log_path = write_activity_log(
            &pane_id,
            "10:00|TaskCreate|#1 A\n10:01|TaskUpdate|in_progress #1\n",
        );

        state.now = 100;
        state.refresh_task_progress();

        // Tasks shown and no inactive timer started
        assert!(state.pane_task_progress(&pane_id).is_some());
        assert!(state.pane_inactive_since(&pane_id).is_none());

        fs::remove_file(&log_path).ok();
    }

    // ─── ScrollState unit tests ─────────────────────────────────────

    #[test]
    fn scroll_state_clamps_to_max() {
        let mut s = ScrollState {
            offset: 0,
            total_lines: 10,
            visible_height: 4,
        };
        s.scroll(100);
        assert_eq!(s.offset, 6); // max = 10 - 4
    }

    #[test]
    fn scroll_state_clamps_to_zero() {
        let mut s = ScrollState {
            offset: 3,
            total_lines: 10,
            visible_height: 4,
        };
        s.scroll(-100);
        assert_eq!(s.offset, 0);
    }

    #[test]
    fn scroll_state_noop_when_content_fits() {
        let mut s = ScrollState {
            offset: 0,
            total_lines: 3,
            visible_height: 5,
        };
        s.scroll(1);
        assert_eq!(s.offset, 0);
    }

    #[test]
    fn scroll_state_exact_fit_no_scroll() {
        let mut s = ScrollState {
            offset: 0,
            total_lines: 5,
            visible_height: 5,
        };
        s.scroll(1);
        assert_eq!(s.offset, 0);
    }

    #[test]
    fn scroll_state_incremental() {
        let mut s = ScrollState {
            offset: 0,
            total_lines: 10,
            visible_height: 4,
        };
        s.scroll(1);
        assert_eq!(s.offset, 1);
        s.scroll(2);
        assert_eq!(s.offset, 3);
        s.scroll(-1);
        assert_eq!(s.offset, 2);
    }

    // ─── apply_git_data tests ───────────────────────────────────────

    #[test]
    fn apply_git_data_copies_all_fields() {
        let mut state = AppState::new("%99".into());
        let data = crate::git::GitData {
            diff_stat: Some((10, 5)),
            branch: "feature/test".into(),
            ahead_behind: Some((2, 1)),
            staged_files: vec![crate::git::GitFileEntry {
                status: 'M',
                name: "lib.rs".into(),
                additions: 10,
                deletions: 5,
                path: String::new(),
            }],
            unstaged_files: vec![],
            untracked_files: vec!["new.rs".into()],
            remote_url: "https://github.com/user/repo".into(),
            pr_number: Some("42".into()),
        };

        state.apply_git_data(data);

        assert_eq!(state.git.diff_stat, Some((10, 5)));
        assert_eq!(state.git.branch, "feature/test");
        assert_eq!(state.git.ahead_behind, Some((2, 1)));
        assert_eq!(state.git.staged_files.len(), 1);
        assert_eq!(state.git.staged_files[0].status, 'M');
        assert!(state.git.unstaged_files.is_empty());
        assert_eq!(state.git.untracked_files, vec!["new.rs"]);
        assert_eq!(state.git.changed_file_count(), 2);
        assert_eq!(state.git.remote_url, "https://github.com/user/repo");
        assert_eq!(state.git.pr_number, Some("42".into()));
    }

    #[test]
    fn apply_git_data_with_defaults() {
        let mut state = AppState::new("%99".into());
        // Pre-fill some state
        state.git.branch = "old-branch".into();
        state.git.pr_number = Some("99".into());

        // Apply empty git data
        state.apply_git_data(crate::git::GitData::default());

        assert_eq!(state.git.diff_stat, None);
        assert!(state.git.branch.is_empty());
        assert_eq!(state.git.ahead_behind, None);
        assert!(state.git.staged_files.is_empty());
        assert!(state.git.unstaged_files.is_empty());
        assert!(state.git.untracked_files.is_empty());
        assert_eq!(state.git.changed_file_count(), 0);
        assert!(state.git.remote_url.is_empty());
        assert_eq!(state.git.pr_number, None);
    }

    #[test]
    fn apply_session_snapshot_rebuilds_derived_state() {
        let mut state = AppState::new("%99".into());
        state.global.selected_pane_row = 3;

        let pane = test_pane("%1");
        let sessions = vec![crate::tmux::SessionInfo {
            session_name: "main".into(),
            windows: vec![crate::tmux::WindowInfo {
                window_id: "@0".into(),
                window_name: "project".into(),
                window_active: true,
                auto_rename: false,
                panes: vec![pane],
            }],
        }];

        state.apply_session_snapshot(true, sessions);

        assert!(state.sidebar_focused);
        assert_eq!(state.repo_groups.len(), 1);
        assert_eq!(state.pane_row_targets.len(), 1);
        assert_eq!(state.global.selected_pane_row, 0);
        // focused_pane_id is set by find_focused_pane() which queries tmux
        // directly, so we don't assert it here (tmux not available in tests).
    }

    // ─── auto_switch_tab tests are in state/tab.rs ────────────────

    // ─── next_bottom_tab / scroll_bottom tests ──────────────────────

    #[test]
    fn next_bottom_tab_toggles() {
        let mut state = AppState::new("%99".into());
        assert_eq!(state.bottom_tab, BottomTab::Activity);
        state.next_bottom_tab();
        assert_eq!(state.bottom_tab, BottomTab::GitStatus);
        state.next_bottom_tab();
        assert_eq!(state.bottom_tab, BottomTab::Activity);
    }

    #[test]
    fn scroll_bottom_dispatches_to_activity() {
        let mut state = AppState::new("%99".into());
        state.bottom_tab = BottomTab::Activity;
        state.activity_scroll = ScrollState {
            offset: 0,
            total_lines: 10,
            visible_height: 3,
        };

        state.scroll_bottom(2);
        assert_eq!(state.activity_scroll.offset, 2);
        assert_eq!(state.git_scroll.offset, 0);
    }

    #[test]
    fn scroll_bottom_dispatches_to_git() {
        let mut state = AppState::new("%99".into());
        state.bottom_tab = BottomTab::GitStatus;
        state.git_scroll = ScrollState {
            offset: 0,
            total_lines: 10,
            visible_height: 3,
        };

        state.scroll_bottom(2);
        assert_eq!(state.git_scroll.offset, 2);
        assert_eq!(state.activity_scroll.offset, 0);
    }

    // ─── handle_mouse_scroll tests ────────────────────────────────────

    #[test]
    fn mouse_scroll_in_bottom_panel_scrolls_activity() {
        let mut state = AppState::new("%99".into());
        state.bottom_tab = BottomTab::Activity;
        state.activity_scroll = ScrollState {
            offset: 0,
            total_lines: 30,
            visible_height: 10,
        };
        // term_height=50, bottom_panel=20 → bottom starts at row 30
        // mouse at row 35 → in bottom panel
        state.handle_mouse_scroll(35, 50, 20, 3);
        assert_eq!(state.activity_scroll.offset, 3);
        assert_eq!(state.panes_scroll.offset, 0);
    }

    #[test]
    fn mouse_scroll_in_agents_panel_scrolls_agents() {
        let mut state = AppState::new("%99".into());
        state.panes_scroll = ScrollState {
            offset: 0,
            total_lines: 40,
            visible_height: 20,
        };
        // term_height=50, bottom_panel=20 → bottom starts at row 30
        // mouse at row 10 → in agents panel
        state.handle_mouse_scroll(10, 50, 20, 3);
        assert_eq!(state.panes_scroll.offset, 3);
        assert_eq!(state.activity_scroll.offset, 0);
    }

    #[test]
    fn mouse_scroll_up_in_agents_panel() {
        let mut state = AppState::new("%99".into());
        state.panes_scroll = ScrollState {
            offset: 5,
            total_lines: 40,
            visible_height: 20,
        };
        state.handle_mouse_scroll(10, 50, 20, -3);
        assert_eq!(state.panes_scroll.offset, 2);
    }

    #[test]
    fn mouse_scroll_at_boundary_row_goes_to_bottom() {
        let mut state = AppState::new("%99".into());
        state.bottom_tab = BottomTab::GitStatus;
        state.git_scroll = ScrollState {
            offset: 0,
            total_lines: 20,
            visible_height: 10,
        };
        // term_height=50, bottom_panel=20 → bottom starts at row 30
        // mouse at exactly row 30 → in bottom panel
        state.handle_mouse_scroll(30, 50, 20, 3);
        assert_eq!(state.git_scroll.offset, 3);
        assert_eq!(state.panes_scroll.offset, 0);
    }

    #[test]
    fn mouse_scroll_just_above_boundary_goes_to_agents() {
        let mut state = AppState::new("%99".into());
        state.panes_scroll = ScrollState {
            offset: 0,
            total_lines: 40,
            visible_height: 20,
        };
        // row 29, just above bottom_start=30
        state.handle_mouse_scroll(29, 50, 20, 3);
        assert_eq!(state.panes_scroll.offset, 3);
        assert_eq!(state.activity_scroll.offset, 0);
    }

    // ─── move_pane_selection edge cases ─────────────────────────────

    #[test]
    fn move_pane_selection_returns_false_when_empty() {
        let mut state = AppState::new("%99".into());
        assert!(!state.move_pane_selection(1));
        assert!(!state.move_pane_selection(-1));
    }

    #[test]
    fn move_pane_selection_boundary_returns() {
        let mut state = AppState::new("%99".into());
        state.pane_row_targets = vec![
            RowTarget {
                pane_id: "%1".into(),
            },
            RowTarget {
                pane_id: "%2".into(),
            },
            RowTarget {
                pane_id: "%3".into(),
            },
        ];
        state.global.selected_pane_row = 0;

        assert!(!state.move_pane_selection(-1), "can't go below 0");
        assert!(state.move_pane_selection(1));
        assert!(state.move_pane_selection(1));
        assert_eq!(state.global.selected_pane_row, 2);
        assert!(!state.move_pane_selection(1), "can't go past end");
    }

    // ─── rebuild_row_targets clamp tests ────────────────────────────

    #[test]
    fn rebuild_row_targets_clamps_selection_when_shrinks() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![RepoGroup {
            name: "project".into(),
            has_focus: true,
            panes: vec![
                (test_pane("%1"), PaneGitInfo::default()),
                (test_pane("%2"), PaneGitInfo::default()),
                (test_pane("%3"), PaneGitInfo::default()),
            ],
        }];
        state.global.selected_pane_row = 2;
        state.rebuild_row_targets();
        assert_eq!(state.global.selected_pane_row, 2);

        // Shrink to 1 pane
        state.repo_groups[0].panes = vec![(test_pane("%1"), PaneGitInfo::default())];
        state.rebuild_row_targets();
        assert_eq!(
            state.global.selected_pane_row, 0,
            "should clamp to last valid index"
        );
    }

    #[test]
    fn rebuild_row_targets_empty_groups() {
        let mut state = AppState::new("%99".into());
        state.global.selected_pane_row = 5;
        state.repo_groups = vec![];
        state.rebuild_row_targets();
        assert!(state.pane_row_targets.is_empty());
        // selected_pane_row stays as-is when targets empty (no clamp needed)
        assert_eq!(state.global.selected_pane_row, 5);
    }

    #[test]
    fn rebuild_row_targets_respects_filter() {
        let mut state = AppState::new("%99".into());
        let mut p1 = test_pane("%1");
        p1.status = PaneStatus::Running;
        let mut p2 = test_pane("%2");
        p2.status = PaneStatus::Idle;
        let mut p3 = test_pane("%3");
        p3.status = PaneStatus::Running;

        state.repo_groups = vec![RepoGroup {
            name: "project".into(),
            has_focus: true,
            panes: vec![
                (p1, PaneGitInfo::default()),
                (p2, PaneGitInfo::default()),
                (p3, PaneGitInfo::default()),
            ],
        }];

        // All filter: all 3 panes
        state.global.status_filter = StatusFilter::All;
        state.rebuild_row_targets();
        assert_eq!(state.pane_row_targets.len(), 3);

        // Running filter: only 2 panes
        state.global.status_filter = StatusFilter::Running;
        state.rebuild_row_targets();
        assert_eq!(state.pane_row_targets.len(), 2);
        assert_eq!(state.pane_row_targets[0].pane_id, "%1");
        assert_eq!(state.pane_row_targets[1].pane_id, "%3");

        // Idle filter: only 1 pane
        state.global.status_filter = StatusFilter::Idle;
        state.rebuild_row_targets();
        assert_eq!(state.pane_row_targets.len(), 1);
        assert_eq!(state.pane_row_targets[0].pane_id, "%2");

        // Error filter: no panes
        state.global.status_filter = StatusFilter::Error;
        state.rebuild_row_targets();
        assert!(state.pane_row_targets.is_empty());
    }

    #[test]
    fn rebuild_row_targets_clamps_cursor_on_filter_change() {
        let mut state = AppState::new("%99".into());
        let mut p1 = test_pane("%1");
        p1.status = PaneStatus::Running;
        let mut p2 = test_pane("%2");
        p2.status = PaneStatus::Idle;
        let mut p3 = test_pane("%3");
        p3.status = PaneStatus::Idle;

        state.repo_groups = vec![RepoGroup {
            name: "project".into(),
            has_focus: true,
            panes: vec![
                (p1, PaneGitInfo::default()),
                (p2, PaneGitInfo::default()),
                (p3, PaneGitInfo::default()),
            ],
        }];

        // Select last agent in All view
        state.global.status_filter = StatusFilter::All;
        state.rebuild_row_targets();
        state.global.selected_pane_row = 2;

        // Switch to Running filter (only 1 pane) — cursor should clamp
        state.global.status_filter = StatusFilter::Running;
        state.rebuild_row_targets();
        assert_eq!(state.global.selected_pane_row, 0);
        assert_eq!(state.pane_row_targets[0].pane_id, "%1");
    }

    // ─── handle_mouse_click tests ────────────────────────────────────

    #[test]
    fn mouse_click_selects_agent_row() {
        let mut state = AppState::new("%99".into());
        state.pane_row_targets = vec![
            RowTarget {
                pane_id: "%1".into(),
            },
            RowTarget {
                pane_id: "%2".into(),
            },
        ];
        // line_to_row: line 0 = group header (None), line 1 = agent 0, line 2 = agent 1
        state.line_to_row = vec![None, Some(0), Some(1)];
        state.panes_scroll.offset = 0;

        // row 0 = filter bar, row 1 = secondary header, row 2+ = agent list rows
        state.handle_mouse_click(3, 5); // row 3 → line_index = (3-2) = 1 → agent row 0
        assert_eq!(state.global.selected_pane_row, 0);

        state.handle_mouse_click(4, 5); // row 4 → line_index = (4-2) = 2 → agent row 1
        assert_eq!(state.global.selected_pane_row, 1);
    }

    #[test]
    fn mouse_click_on_filter_bar_changes_filter() {
        let mut state = AppState::new("%99".into());
        state.pane_row_targets = vec![RowTarget {
            pane_id: "%1".into(),
        }];
        state.line_to_row = vec![None, Some(0)];
        state.global.selected_pane_row = 0;
        state.global.status_filter = StatusFilter::All;

        // Click on "All" (x=1..3) should keep All
        reset_filter_debounce(&mut state);
        state.handle_mouse_click(0, 1);
        assert_eq!(state.global.status_filter, StatusFilter::All);

        // Click on Running icon area (x=6..) should switch to Running
        reset_filter_debounce(&mut state);
        state.handle_mouse_click(0, 6);
        assert_eq!(state.global.status_filter, StatusFilter::Running);

        // agent selection unchanged
        assert_eq!(state.global.selected_pane_row, 0);
    }

    #[test]
    fn mouse_click_on_secondary_header_toggles_repo_popup() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![
            RepoGroup {
                name: "alpha".into(),
                has_focus: true,
                panes: vec![(test_pane("%1"), PaneGitInfo::default())],
            },
            RepoGroup {
                name: "beta".into(),
                has_focus: false,
                panes: vec![(test_pane("%2"), PaneGitInfo::default())],
            },
        ];
        state.repo_button_col = Some(20);

        state.handle_mouse_click(1, 19);
        assert!(!state.is_repo_popup_open());

        state.handle_mouse_click(1, 20);
        assert!(state.is_repo_popup_open());
    }

    #[test]
    fn mouse_click_with_scroll_offset() {
        let mut state = AppState::new("%99".into());
        state.pane_row_targets = vec![
            RowTarget {
                pane_id: "%1".into(),
            },
            RowTarget {
                pane_id: "%2".into(),
            },
        ];
        // 5 lines total, scrolled down by 2
        state.line_to_row = vec![None, Some(0), Some(0), None, Some(1)];
        state.panes_scroll.offset = 2;

        // row 4 → line_index = (4-2) + 2 = 4 → agent row 1
        state.handle_mouse_click(4, 5);
        assert_eq!(state.global.selected_pane_row, 1);
    }

    #[test]
    fn mouse_click_out_of_bounds() {
        let mut state = AppState::new("%99".into());
        state.pane_row_targets = vec![RowTarget {
            pane_id: "%1".into(),
        }];
        state.line_to_row = vec![None, Some(0)];
        state.global.selected_pane_row = 0;

        state.handle_mouse_click(50, 5); // way beyond line_to_row
        assert_eq!(state.global.selected_pane_row, 0); // unchanged
    }

    // ─── StatusFilter tests ───────────────────────────────────────────

    #[test]
    fn status_filter_next_cycles() {
        assert_eq!(StatusFilter::All.next(), StatusFilter::Running);
        assert_eq!(StatusFilter::Running.next(), StatusFilter::Waiting);
        assert_eq!(StatusFilter::Waiting.next(), StatusFilter::Idle);
        assert_eq!(StatusFilter::Idle.next(), StatusFilter::Error);
        assert_eq!(StatusFilter::Error.next(), StatusFilter::All);
    }

    #[test]
    fn status_filter_prev_cycles() {
        assert_eq!(StatusFilter::All.prev(), StatusFilter::Error);
        assert_eq!(StatusFilter::Error.prev(), StatusFilter::Idle);
        assert_eq!(StatusFilter::Idle.prev(), StatusFilter::Waiting);
        assert_eq!(StatusFilter::Waiting.prev(), StatusFilter::Running);
        assert_eq!(StatusFilter::Running.prev(), StatusFilter::All);
    }

    #[test]
    fn status_filter_matches_status() {
        assert!(StatusFilter::All.matches(&PaneStatus::Running));
        assert!(StatusFilter::All.matches(&PaneStatus::Idle));
        assert!(StatusFilter::All.matches(&PaneStatus::Waiting));
        assert!(StatusFilter::All.matches(&PaneStatus::Error));

        assert!(StatusFilter::Running.matches(&PaneStatus::Running));
        assert!(!StatusFilter::Running.matches(&PaneStatus::Idle));
        assert!(!StatusFilter::Running.matches(&PaneStatus::Waiting));
        assert!(!StatusFilter::Running.matches(&PaneStatus::Error));

        assert!(StatusFilter::Waiting.matches(&PaneStatus::Waiting));
        assert!(!StatusFilter::Waiting.matches(&PaneStatus::Running));

        assert!(StatusFilter::Idle.matches(&PaneStatus::Idle));
        assert!(!StatusFilter::Idle.matches(&PaneStatus::Running));

        assert!(StatusFilter::Error.matches(&PaneStatus::Error));
        assert!(!StatusFilter::Error.matches(&PaneStatus::Idle));
    }

    // ─── status_counts tests ─────────────────────────────────────────

    #[test]
    fn status_counts_empty() {
        let state = AppState::new("%99".into());
        assert_eq!(state.status_counts(), (0, 0, 0, 0, 0));
    }

    #[test]
    fn status_counts_mixed() {
        let mut state = AppState::new("%99".into());
        let mut p1 = test_pane("%1");
        p1.status = PaneStatus::Running;
        let mut p2 = test_pane("%2");
        p2.status = PaneStatus::Running;
        let mut p3 = test_pane("%3");
        p3.status = PaneStatus::Idle;
        let mut p4 = test_pane("%4");
        p4.status = PaneStatus::Waiting;
        let mut p5 = test_pane("%5");
        p5.status = PaneStatus::Error;

        state.repo_groups = vec![RepoGroup {
            name: "project".into(),
            has_focus: true,
            panes: vec![
                (p1, PaneGitInfo::default()),
                (p2, PaneGitInfo::default()),
                (p3, PaneGitInfo::default()),
                (p4, PaneGitInfo::default()),
                (p5, PaneGitInfo::default()),
            ],
        }];
        // (all, running, waiting, idle, error)
        assert_eq!(state.status_counts(), (5, 2, 1, 1, 1));
    }

    // ─── handle_filter_click tests ───────────────────────────────────

    #[test]
    fn filter_click_all_positions() {
        let mut state = AppState::new("%99".into());
        // With 0 agents, counts are all 0, so layout: " All  ●0  ◐0  ○0  ✕0"
        //                                              0123456789...

        // "All" at x=1..3
        state.global.status_filter = StatusFilter::Running;
        reset_filter_debounce(&mut state);
        state.handle_filter_click(1);
        assert_eq!(state.global.status_filter, StatusFilter::All);

        reset_filter_debounce(&mut state);
        state.handle_filter_click(3);
        assert_eq!(state.global.status_filter, StatusFilter::All);

        // "●0" at x=6..7
        reset_filter_debounce(&mut state);
        state.handle_filter_click(6);
        assert_eq!(state.global.status_filter, StatusFilter::Running);

        // "◐0" at x=10..11
        reset_filter_debounce(&mut state);
        state.handle_filter_click(10);
        assert_eq!(state.global.status_filter, StatusFilter::Waiting);

        // "○0" at x=14..15
        reset_filter_debounce(&mut state);
        state.handle_filter_click(14);
        assert_eq!(state.global.status_filter, StatusFilter::Idle);

        // "✕0" at x=18..19
        reset_filter_debounce(&mut state);
        state.handle_filter_click(18);
        assert_eq!(state.global.status_filter, StatusFilter::Error);
    }

    #[test]
    fn filter_click_gap_does_nothing() {
        let mut state = AppState::new("%99".into());
        state.global.status_filter = StatusFilter::All;

        // x=0 is leading space, x=4 and x=5 are separator
        state.handle_filter_click(0);
        assert_eq!(state.global.status_filter, StatusFilter::All);

        state.handle_filter_click(4);
        assert_eq!(state.global.status_filter, StatusFilter::All);

        state.handle_filter_click(5);
        assert_eq!(state.global.status_filter, StatusFilter::All);
    }

    #[test]
    fn filter_click_debounce_ignores_rapid_clicks() {
        let mut state = AppState::new("%99".into());
        state.global.status_filter = StatusFilter::All;

        // First click within debounce window should be ignored
        // (AppState::new sets last_filter_click to now)
        state.handle_filter_click(6); // would be Running
        assert_eq!(state.global.status_filter, StatusFilter::All); // unchanged due to debounce

        // After resetting debounce, click should work
        reset_filter_debounce(&mut state);
        state.handle_filter_click(6);
        assert_eq!(state.global.status_filter, StatusFilter::Running);

        // Immediate second click should be debounced
        state.handle_filter_click(1); // would be All
        assert_eq!(state.global.status_filter, StatusFilter::Running); // unchanged
    }

    #[test]
    fn filter_click_with_large_counts() {
        let mut state = AppState::new("%99".into());
        // Add 10 running agents to shift positions
        let panes: Vec<_> = (0..10)
            .map(|i| {
                let mut p = test_pane(&format!("%{i}"));
                p.status = PaneStatus::Running;
                (p, PaneGitInfo::default())
            })
            .collect();
        state.repo_groups = vec![RepoGroup {
            name: "project".into(),
            has_focus: true,
            panes,
        }];
        // Layout: " All  ●10  ◐0  ○0  ✕0"
        //          0123456789...
        // "●10" at x=6..8 (icon + "10")
        reset_filter_debounce(&mut state);
        state.handle_filter_click(6);
        assert_eq!(state.global.status_filter, StatusFilter::Running);
        reset_filter_debounce(&mut state);
        state.handle_filter_click(8);
        assert_eq!(state.global.status_filter, StatusFilter::Running);

        // "◐0" shifts to x=11..12
        reset_filter_debounce(&mut state);
        state.handle_filter_click(11);
        assert_eq!(state.global.status_filter, StatusFilter::Waiting);
    }

    #[test]
    fn filter_click_rebuilds_row_targets() {
        let mut state = AppState::new("%99".into());
        let mut p1 = test_pane("%1");
        p1.status = PaneStatus::Running;
        let mut p2 = test_pane("%2");
        p2.status = PaneStatus::Idle;
        let mut p3 = test_pane("%3");
        p3.status = PaneStatus::Running;
        state.repo_groups = vec![RepoGroup {
            name: "project".into(),
            has_focus: true,
            panes: vec![
                (p1, PaneGitInfo::default()),
                (p2, PaneGitInfo::default()),
                (p3, PaneGitInfo::default()),
            ],
        }];
        state.global.status_filter = StatusFilter::All;
        state.rebuild_row_targets();
        assert_eq!(state.pane_row_targets.len(), 3);

        // Click Running filter — row_targets should update immediately
        // Layout: " All  ●2  ◐0  ○1  ✕0" → Running at x=6
        reset_filter_debounce(&mut state);
        state.handle_filter_click(6);
        assert_eq!(state.global.status_filter, StatusFilter::Running);
        assert_eq!(state.pane_row_targets.len(), 2);
        assert_eq!(state.pane_row_targets[0].pane_id, "%1");
        assert_eq!(state.pane_row_targets[1].pane_id, "%3");

        // Click Idle filter — row_targets should update again
        // Layout: " All  ●2  ◐0  ○1  ✕0" → Idle at x=14
        reset_filter_debounce(&mut state);
        state.handle_filter_click(14);
        assert_eq!(state.global.status_filter, StatusFilter::Idle);
        assert_eq!(state.pane_row_targets.len(), 1);
        assert_eq!(state.pane_row_targets[0].pane_id, "%2");
    }

    // ─── StatusFilter as_str / from_str tests ─────────────────────────

    #[test]
    fn status_filter_as_str_all_variants() {
        assert_eq!(StatusFilter::All.as_str(), "all");
        assert_eq!(StatusFilter::Running.as_str(), "running");
        assert_eq!(StatusFilter::Waiting.as_str(), "waiting");
        assert_eq!(StatusFilter::Idle.as_str(), "idle");
        assert_eq!(StatusFilter::Error.as_str(), "error");
    }

    #[test]
    fn status_filter_from_str_all_variants() {
        assert_eq!(StatusFilter::from_label("all"), StatusFilter::All);
        assert_eq!(StatusFilter::from_label("running"), StatusFilter::Running);
        assert_eq!(StatusFilter::from_label("waiting"), StatusFilter::Waiting);
        assert_eq!(StatusFilter::from_label("idle"), StatusFilter::Idle);
        assert_eq!(StatusFilter::from_label("error"), StatusFilter::Error);
    }

    #[test]
    fn status_filter_from_str_unknown_defaults_to_all() {
        assert_eq!(StatusFilter::from_label(""), StatusFilter::All);
        assert_eq!(StatusFilter::from_label("unknown"), StatusFilter::All);
        assert_eq!(StatusFilter::from_label("Running"), StatusFilter::All); // case-sensitive
    }

    #[test]
    fn status_filter_roundtrip() {
        for filter in StatusFilter::VARIANTS {
            assert_eq!(StatusFilter::from_label(filter.as_str()), filter);
        }
    }

    // ─── RepoFilter tests ─────────────────────────────────────

    #[test]
    fn repo_filter_persistence_roundtrip() {
        assert_eq!(RepoFilter::from_label("all"), RepoFilter::All);
        assert_eq!(RepoFilter::from_label(""), RepoFilter::All);
        assert_eq!(
            RepoFilter::from_label("my-app"),
            RepoFilter::Repo("my-app".into())
        );
        assert_eq!(RepoFilter::All.as_str(), "all");
        assert_eq!(RepoFilter::Repo("my-app".into()).as_str(), "my-app");
    }

    #[test]
    fn repo_filter_matches_group() {
        assert!(RepoFilter::All.matches_group("anything"));
        assert!(RepoFilter::Repo("app".into()).matches_group("app"));
        assert!(!RepoFilter::Repo("app".into()).matches_group("other"));
    }

    #[test]
    fn repo_filter_all_shows_all_groups() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![
            RepoGroup {
                name: "dotfiles".into(),
                has_focus: true,
                panes: vec![(test_pane("%1"), PaneGitInfo::default())],
            },
            RepoGroup {
                name: "app".into(),
                has_focus: false,
                panes: vec![(test_pane("%2"), PaneGitInfo::default())],
            },
        ];
        state.global.repo_filter = RepoFilter::All;
        state.rebuild_row_targets();

        assert_eq!(state.pane_row_targets.len(), 2);
    }

    #[test]
    fn repo_filter_specific_repo() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![
            RepoGroup {
                name: "dotfiles".into(),
                has_focus: true,
                panes: vec![(test_pane("%1"), PaneGitInfo::default())],
            },
            RepoGroup {
                name: "app".into(),
                has_focus: false,
                panes: vec![(test_pane("%2"), PaneGitInfo::default())],
            },
        ];
        state.global.repo_filter = RepoFilter::Repo("app".into());
        state.rebuild_row_targets();

        assert_eq!(state.pane_row_targets.len(), 1);
        assert_eq!(state.pane_row_targets[0].pane_id, "%2");
    }

    #[test]
    fn repo_filter_combined_with_status() {
        let mut state = AppState::new("%99".into());
        let mut idle_pane = test_pane("%3");
        idle_pane.status = PaneStatus::Idle;
        state.repo_groups = vec![
            RepoGroup {
                name: "app".into(),
                has_focus: true,
                panes: vec![
                    (test_pane("%1"), PaneGitInfo::default()), // Running
                    (idle_pane, PaneGitInfo::default()),       // Idle
                ],
            },
            RepoGroup {
                name: "lib".into(),
                has_focus: false,
                panes: vec![(test_pane("%2"), PaneGitInfo::default())], // Running
            },
        ];
        state.global.repo_filter = RepoFilter::Repo("app".into());
        state.global.status_filter = StatusFilter::Running;
        state.rebuild_row_targets();

        // Only Running panes in "app" group
        assert_eq!(state.pane_row_targets.len(), 1);
        assert_eq!(state.pane_row_targets[0].pane_id, "%1");
    }

    #[test]
    fn repo_filter_stale_name_resets() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![RepoGroup {
            name: "app".into(),
            has_focus: true,
            panes: vec![(test_pane("%1"), PaneGitInfo::default())],
        }];
        state.global.repo_filter = RepoFilter::Repo("deleted-repo".into());
        state.rebuild_row_targets();

        assert_eq!(state.global.repo_filter, RepoFilter::All);
        assert_eq!(state.pane_row_targets.len(), 1);
    }

    #[test]
    fn repo_names_returns_all_plus_groups() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![
            RepoGroup {
                name: "alpha".into(),
                has_focus: true,
                panes: vec![],
            },
            RepoGroup {
                name: "beta".into(),
                has_focus: false,
                panes: vec![],
            },
        ];
        assert_eq!(state.repo_names(), vec!["All", "alpha", "beta"]);
    }

    #[test]
    fn toggle_repo_popup_sets_selected_to_current() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![
            RepoGroup {
                name: "alpha".into(),
                has_focus: true,
                panes: vec![],
            },
            RepoGroup {
                name: "beta".into(),
                has_focus: false,
                panes: vec![],
            },
        ];

        // Default: All → selected should be 0
        state.toggle_repo_popup();
        assert!(state.is_repo_popup_open());
        assert_eq!(state.repo_popup_selected(), 0);

        // Close and set filter to "beta" → selected should be 2
        state.close_repo_popup();
        state.global.repo_filter = RepoFilter::Repo("beta".into());
        state.toggle_repo_popup();
        assert_eq!(state.repo_popup_selected(), 2); // ["All", "alpha", "beta"]
    }

    #[test]
    fn confirm_repo_popup_sets_filter() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![
            RepoGroup {
                name: "alpha".into(),
                has_focus: true,
                panes: vec![(test_pane("%1"), PaneGitInfo::default())],
            },
            RepoGroup {
                name: "beta".into(),
                has_focus: false,
                panes: vec![(test_pane("%2"), PaneGitInfo::default())],
            },
        ];
        state.popup = PopupState::Repo {
            selected: 2, // "beta"
            area: None,
        };
        state.confirm_repo_popup();

        assert_eq!(state.global.repo_filter, RepoFilter::Repo("beta".into()));
        assert!(!state.is_repo_popup_open());
        assert_eq!(state.pane_row_targets.len(), 1);
        assert_eq!(state.pane_row_targets[0].pane_id, "%2");
    }

    #[test]
    fn confirm_repo_popup_all_resets_filter() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![RepoGroup {
            name: "app".into(),
            has_focus: true,
            panes: vec![(test_pane("%1"), PaneGitInfo::default())],
        }];
        state.global.repo_filter = RepoFilter::Repo("app".into());
        state.popup = PopupState::Repo {
            selected: 0, // "All"
            area: None,
        };
        state.confirm_repo_popup();

        assert_eq!(state.global.repo_filter, RepoFilter::All);
    }

    #[test]
    fn status_counts_respects_repo_filter() {
        let mut state = AppState::new("%99".into());
        let mut idle_pane = test_pane("%2");
        idle_pane.status = PaneStatus::Idle;
        state.repo_groups = vec![
            RepoGroup {
                name: "app".into(),
                has_focus: true,
                panes: vec![(test_pane("%1"), PaneGitInfo::default())], // Running
            },
            RepoGroup {
                name: "lib".into(),
                has_focus: false,
                panes: vec![(idle_pane, PaneGitInfo::default())], // Idle
            },
        ];

        // All repos: 2 total
        state.global.repo_filter = RepoFilter::All;
        let (all, running, _, idle, _) = state.status_counts();
        assert_eq!(all, 2);
        assert_eq!(running, 1);
        assert_eq!(idle, 1);

        // Filter to "app" only: 1 Running
        state.global.repo_filter = RepoFilter::Repo("app".into());
        let (all, running, _, idle, _) = state.status_counts();
        assert_eq!(all, 1);
        assert_eq!(running, 1);
        assert_eq!(idle, 0);
    }
}
