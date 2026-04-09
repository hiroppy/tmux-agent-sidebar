use std::collections::HashMap;
use std::time::Instant;

use crate::activity::{ActivityEntry, TaskProgress};
use crate::tmux::{self, SessionInfo};
use crate::ui::colors::ColorTheme;

mod refresh;
mod tab;
#[cfg(test)]
pub(crate) use refresh::{TaskProgressDecision, classify_task_progress};

#[derive(Debug, Clone, PartialEq)]
pub enum Focus {
    Filter,
    Agents,
    ActivityLog,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AgentFilter {
    All,
    Running,
    Waiting,
    Idle,
    Error,
}

impl AgentFilter {
    pub const VARIANTS: [AgentFilter; 5] = [
        AgentFilter::All,
        AgentFilter::Running,
        AgentFilter::Waiting,
        AgentFilter::Idle,
        AgentFilter::Error,
    ];

    pub fn next(self) -> Self {
        let idx = AgentFilter::VARIANTS
            .iter()
            .position(|v| *v == self)
            .unwrap_or(0);
        AgentFilter::VARIANTS[(idx + 1) % AgentFilter::VARIANTS.len()]
    }

    pub fn prev(self) -> Self {
        let idx = AgentFilter::VARIANTS
            .iter()
            .position(|v| *v == self)
            .unwrap_or(0);
        AgentFilter::VARIANTS[(idx + AgentFilter::VARIANTS.len() - 1) % AgentFilter::VARIANTS.len()]
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

    pub fn from_str(s: &str) -> Self {
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
            AgentFilter::All => true,
            AgentFilter::Running => *status == crate::tmux::PaneStatus::Running,
            AgentFilter::Waiting => *status == crate::tmux::PaneStatus::Waiting,
            AgentFilter::Idle => *status == crate::tmux::PaneStatus::Idle,
            AgentFilter::Error => *status == crate::tmux::PaneStatus::Error,
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

    pub fn from_str(s: &str) -> Self {
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

#[derive(Debug, Clone)]
pub struct RowTarget {
    pub pane_id: String,
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
    pub agent_filter: AgentFilter,
    pub selected_agent_row: usize,
    pub repo_filter: RepoFilter,
    /// Last filter value successfully written to tmux.
    last_saved_filter: AgentFilter,
    /// Last cursor value successfully written to tmux.
    last_saved_cursor: usize,
    /// Last repo filter value successfully written to tmux.
    last_saved_repo_filter: RepoFilter,
}

impl GlobalState {
    pub fn new() -> Self {
        Self {
            agent_filter: AgentFilter::All,
            selected_agent_row: 0,
            repo_filter: RepoFilter::All,
            last_saved_filter: AgentFilter::All,
            last_saved_cursor: 0,
            last_saved_repo_filter: RepoFilter::All,
        }
    }

    /// Save filter to tmux global variable.
    /// Only updates `last_saved_filter` on success so that a failed write
    /// does not cause sync to overwrite the user's choice.
    pub fn save_filter(&mut self) {
        if tmux::run_tmux(&["set", "-g", "@sidebar_filter", self.agent_filter.as_str()]).is_some() {
            self.last_saved_filter = self.agent_filter;
        }
    }

    /// Save cursor position to tmux global variable.
    pub fn save_cursor(&mut self) {
        if tmux::run_tmux(&[
            "set",
            "-g",
            "@sidebar_cursor",
            &self.selected_agent_row.to_string(),
        ])
        .is_some()
        {
            self.last_saved_cursor = self.selected_agent_row;
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
            let tmux_filter = AgentFilter::from_str(filter_str);
            if tmux_filter != self.last_saved_filter {
                self.agent_filter = tmux_filter;
                self.last_saved_filter = tmux_filter;
            }
        }
        if let Some(cursor_str) = opts.get("@sidebar_cursor")
            && let Ok(n) = cursor_str.parse::<usize>()
            && n != self.last_saved_cursor
        {
            self.selected_agent_row = n;
            self.last_saved_cursor = n;
        }
        if let Some(repo_str) = opts.get("@sidebar_repo_filter") {
            let tmux_repo = RepoFilter::from_str(repo_str);
            if tmux_repo != self.last_saved_repo_filter {
                self.repo_filter = tmux_repo.clone();
                self.last_saved_repo_filter = tmux_repo;
            }
        }
    }
}

pub struct AppState {
    pub now: u64,
    pub sessions: Vec<SessionInfo>,
    pub repo_groups: Vec<crate::group::RepoGroup>,
    pub sidebar_focused: bool,
    pub focus: Focus,
    pub spinner_frame: usize,
    pub agent_row_targets: Vec<RowTarget>,
    pub activity_entries: Vec<ActivityEntry>,
    pub activity_scroll: ScrollState,
    pub focused_pane_id: Option<String>,
    pub tmux_pane: String,
    pub activity_max_entries: usize,
    pub line_to_row: Vec<Option<usize>>,
    pub agents_scroll: ScrollState,
    pub theme: ColorTheme,
    pub bottom_tab: BottomTab,
    pub git: crate::git::GitData,
    pub git_scroll: ScrollState,
    pub pane_ports: HashMap<String, Vec<u16>>,
    pub pane_task_progress: HashMap<String, TaskProgress>,
    pub pane_task_dismissed: HashMap<String, usize>,
    /// Tracks when each pane first became inactive (epoch seconds).
    pub pane_inactive_since: HashMap<String, u64>,
    /// Agent pane IDs that have already been seen.
    pub seen_agent_panes: std::collections::HashSet<String>,
    /// Per-pane bottom tab preference.
    pub pane_tab_prefs: HashMap<String, BottomTab>,
    /// Previous focused pane ID, used to detect focus changes.
    pub prev_focused_pane_id: Option<String>,
    /// Last time a mouse click was processed on the filter bar (for debounce).
    pub last_filter_click: std::time::Instant,
    pub repo_popup_open: bool,
    pub repo_popup_selected: usize,
    pub repo_popup_area: Option<ratatui::layout::Rect>,
    pub repo_button_col: u16,
    pub cat_state: crate::ui::cat::CatState,
    /// Cat animation X position (character offset from left of bottom panel).
    pub cat_x: u16,
    /// Cat animation frame index (0 = sitting, 1-2 = running).
    pub cat_frame: usize,
    pub cat_bob_timer: usize,
    /// Update notice shown when a newer GitHub release is available.
    pub version_notice: Option<crate::version::UpdateNotice>,
    /// Shared state across sidebar instances, persisted to tmux global variables.
    pub global: GlobalState,
    /// Hyperlink overlays to be written after frame render (OSC 8).
    pub hyperlink_overlays: Vec<HyperlinkOverlay>,
    pub port_scan_initialized: bool,
    pub last_port_refresh: Instant,
}

/// Screen-positioned hyperlink overlay for OSC 8 terminal hyperlinks.
#[derive(Debug, Clone)]
pub struct HyperlinkOverlay {
    pub x: u16,
    pub y: u16,
    pub text: String,
    pub url: String,
}

impl AppState {
    pub fn new(tmux_pane: String) -> Self {
        Self {
            now: 0,
            sessions: vec![],
            repo_groups: vec![],
            sidebar_focused: false,
            focus: Focus::Agents,
            spinner_frame: 0,
            agent_row_targets: vec![],
            activity_entries: vec![],
            activity_scroll: ScrollState::default(),
            focused_pane_id: None,
            tmux_pane,
            activity_max_entries: 50,
            line_to_row: vec![],
            agents_scroll: ScrollState::default(),
            theme: ColorTheme::default(),
            bottom_tab: BottomTab::Activity,
            git: crate::git::GitData::default(),
            git_scroll: ScrollState::default(),
            pane_ports: HashMap::new(),
            pane_task_progress: HashMap::new(),
            pane_task_dismissed: HashMap::new(),
            pane_inactive_since: HashMap::new(),
            seen_agent_panes: std::collections::HashSet::new(),
            pane_tab_prefs: HashMap::new(),
            prev_focused_pane_id: None,
            last_filter_click: std::time::Instant::now(),
            repo_popup_open: false,
            repo_popup_selected: 0,
            repo_popup_area: None,
            repo_button_col: u16::MAX,
            cat_state: crate::ui::cat::CatState::Idle,
            cat_x: crate::ui::cat::CAT_HOME_X,
            cat_frame: 0,
            cat_bob_timer: 0,
            version_notice: None,
            global: GlobalState::new(),
            hyperlink_overlays: vec![],
            port_scan_initialized: false,
            last_port_refresh: Instant::now(),
        }
    }

    pub fn rebuild_row_targets(&mut self) {
        // Reset stale repo filter if the repo no longer exists
        if let RepoFilter::Repo(ref name) = self.global.repo_filter {
            if !self.repo_groups.iter().any(|g| g.name == *name) {
                self.global.repo_filter = RepoFilter::All;
            }
        }

        self.agent_row_targets.clear();
        for group in &self.repo_groups {
            if !self.global.repo_filter.matches_group(&group.name) {
                continue;
            }
            for (pane, _) in &group.panes {
                if self.global.agent_filter.matches(&pane.status) {
                    self.agent_row_targets.push(RowTarget {
                        pane_id: pane.pane_id.clone(),
                    });
                }
            }
        }
        if self.global.selected_agent_row >= self.agent_row_targets.len()
            && !self.agent_row_targets.is_empty()
        {
            self.global.selected_agent_row = self.agent_row_targets.len() - 1;
        }
    }

    pub fn find_focused_pane(&mut self) {
        // Query tmux directly for the active pane, not through self.sessions
        // which only contains agent panes. This allows activity/git info to
        // be displayed even when the focused pane has no agent running.
        // When the sidebar has focus, find_active_pane returns None — preserve
        // the previously focused pane so bottom panel data stays stable.
        if let Some((id, _)) = tmux::find_active_pane(&self.tmux_pane) {
            self.focused_pane_id = Some(id);
        }
    }

    /// Move agent selection. Returns true if moved, false if at boundary.
    pub fn move_agent_selection(&mut self, delta: isize) -> bool {
        if self.agent_row_targets.is_empty() {
            return false;
        }
        let len = self.agent_row_targets.len() as isize;
        let next = self.global.selected_agent_row as isize + delta;
        if next >= 0 && next < len {
            self.global.selected_agent_row = next as usize;
            true
        } else {
            false
        }
    }

    pub fn activate_selection(&self) {
        if let Some(target) = self.agent_row_targets.get(self.global.selected_agent_row) {
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
            self.agents_scroll.scroll(delta);
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

        let (_, running, waiting, idle, error) = self.status_counts();
        // Layout: " All  ●N  ◐N  ○N  ✕N"
        // Calculate x ranges for each filter item
        let mut x = 1usize; // leading space
        let items: Vec<(AgentFilter, usize)> = vec![
            (AgentFilter::All, 3),                                  // "All"
            (AgentFilter::Running, 1 + format!("{running}").len()), // icon + count
            (AgentFilter::Waiting, 1 + format!("{waiting}").len()),
            (AgentFilter::Idle, 1 + format!("{idle}").len()),
            (AgentFilter::Error, 1 + format!("{error}").len()),
        ];
        let col = col as usize;
        // Check if click is on repo button (right side)
        if col >= self.repo_button_col as usize {
            self.toggle_repo_popup();
            return;
        }
        for (i, (filter, width)) in items.iter().enumerate() {
            if i > 0 {
                x += 2; // "  " separator
            }
            if col >= x && col < x + width {
                self.global.agent_filter = *filter;
                self.global.save_filter();
                self.rebuild_row_targets();
                return;
            }
            x += width;
        }
    }

    /// Handle mouse click in agents panel. Maps screen row to agent row
    /// via line_to_row (adjusted for scroll offset) and activates that pane.
    /// Row 0 is the fixed filter bar, row 1+ maps to the scrollable agent list.
    pub fn handle_mouse_click(&mut self, row: u16, col: u16) {
        // Handle popup interactions first
        if self.repo_popup_open {
            if let Some(popup_area) = self.repo_popup_area {
                if row >= popup_area.y
                    && row < popup_area.y + popup_area.height
                    && col >= popup_area.x
                    && col < popup_area.x + popup_area.width
                {
                    // Click inside popup — select item (subtract 1 for top border)
                    let item_index = (row - popup_area.y).saturating_sub(1) as usize;
                    let repos = self.repo_names();
                    if item_index < repos.len() {
                        self.repo_popup_selected = item_index;
                        self.confirm_repo_popup();
                    }
                    return;
                }
            }
            // Click outside popup — close it
            self.close_repo_popup();
            return;
        }

        if row == 0 {
            self.handle_filter_click(col);
            return;
        }
        let line_index = (row as usize - 1) + self.agents_scroll.offset;
        if let Some(Some(agent_row)) = self.line_to_row.get(line_index) {
            self.global.selected_agent_row = *agent_row;
            self.global.save_cursor();
            self.activate_selection();
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
        self.repo_popup_open = !self.repo_popup_open;
        if self.repo_popup_open {
            // Set selected to current filter position
            let names = self.repo_names();
            self.repo_popup_selected = match &self.global.repo_filter {
                RepoFilter::All => 0,
                RepoFilter::Repo(name) => names.iter().position(|n| n == name).unwrap_or(0),
            };
        }
    }

    pub fn confirm_repo_popup(&mut self) {
        let names = self.repo_names();
        if let Some(name) = names.get(self.repo_popup_selected) {
            self.global.repo_filter = if self.repo_popup_selected == 0 {
                RepoFilter::All
            } else {
                RepoFilter::Repo(name.clone())
            };
        }
        self.repo_popup_open = false;
        self.global.save_repo_filter();
        self.rebuild_row_targets();
    }

    pub fn close_repo_popup(&mut self) {
        self.repo_popup_open = false;
    }

    /// Advance cat animation state. Called every spinner tick (200ms).
    pub fn tick_cat(&mut self, panel_width: u16) {
        let running_count = self
            .repo_groups
            .iter()
            .flat_map(|g| &g.panes)
            .filter(|(p, _)| p.status == crate::tmux::PaneStatus::Running)
            .count();

        let desk_x = panel_width.saturating_sub(
            crate::ui::cat::DESK_OFFSET + crate::ui::cat::DESK_WIDTH + crate::ui::cat::CAT_WIDTH,
        );

        match self.cat_state {
            crate::ui::cat::CatState::Idle => {
                if running_count > 0 {
                    self.cat_state = crate::ui::cat::CatState::WalkRight;
                    self.cat_frame = 0;
                    self.cat_x = self.cat_x.saturating_add(1);
                } else {
                    self.cat_bob_timer = (self.cat_bob_timer + 1) % crate::ui::cat::BOB_INTERVAL;
                }
            }
            crate::ui::cat::CatState::WalkRight => {
                self.cat_x = self.cat_x.saturating_add(1);
                self.cat_frame = if self.cat_frame == 1 { 2 } else { 1 };
                if self.cat_x >= desk_x {
                    self.cat_x = desk_x;
                    self.cat_state = crate::ui::cat::CatState::Working;
                    self.cat_frame = 0;
                }
            }
            crate::ui::cat::CatState::Working => {
                self.cat_frame = if self.cat_frame == 1 { 2 } else { 1 };
                if running_count == 0 {
                    self.cat_state = crate::ui::cat::CatState::WalkLeft;
                    self.cat_frame = 0;
                }
            }
            crate::ui::cat::CatState::WalkLeft => {
                self.cat_x = self.cat_x.saturating_sub(1);
                self.cat_frame = if self.cat_frame == 1 { 2 } else { 1 };
                if self.cat_x <= crate::ui::cat::CAT_HOME_X {
                    self.cat_x = crate::ui::cat::CAT_HOME_X;
                    self.cat_state = crate::ui::cat::CatState::Idle;
                    self.cat_frame = 0;
                    self.cat_bob_timer = 0;
                }
            }
        }
    }
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

    fn test_pane(id: &str) -> PaneInfo {
        PaneInfo {
            pane_id: id.into(),
            pane_active: false,
            status: PaneStatus::Running,
            attention: false,
            agent: AgentType::Claude,
            path: "/tmp".into(),
            prompt: String::new(),
            prompt_is_response: false,
            started_at: None,
            wait_reason: String::new(),
            permission_mode: PermissionMode::Default,
            subagents: vec![],
            pane_pid: None,
            worktree_name: String::new(),
            worktree_branch: String::new(),
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

        assert_eq!(state.agent_row_targets.len(), 3);
        assert_eq!(state.agent_row_targets[0].pane_id, "%1");
        assert_eq!(state.agent_row_targets[1].pane_id, "%2");
        assert_eq!(state.agent_row_targets[2].pane_id, "%3");
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
        assert_eq!(state.global.selected_agent_row, 0);
        assert_eq!(state.agent_row_targets[0].pane_id, "%1");

        // Move to second group
        assert!(state.move_agent_selection(1));
        assert_eq!(state.global.selected_agent_row, 1);
        assert_eq!(state.agent_row_targets[1].pane_id, "%5");
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
        assert!(state.pane_task_progress.get(&pane_id).is_none());
        // Dismissed count should be recorded
        assert_eq!(state.pane_task_dismissed.get(&pane_id), Some(&2));

        // Calling refresh again should still be hidden (no flicker)
        state.refresh_task_progress();
        assert!(state.pane_task_progress.get(&pane_id).is_none());

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
        assert!(state.pane_task_progress.get(&pane_id).is_none());

        // Now add a new in-progress task → should re-show
        fs::write(
            &log_path,
            "10:00|TaskCreate|#1 A\n10:01|TaskUpdate|completed #1\n10:02|TaskCreate|#2 B\n10:03|TaskUpdate|in_progress #2\n",
        ).unwrap();
        state.refresh_task_progress();
        assert!(state.pane_task_progress.get(&pane_id).is_some());

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
        state.pane_task_progress.insert(
            pane_id.clone(),
            TaskProgress {
                tasks: vec![("stale".into(), TaskStatus::InProgress)],
            },
        );
        state.pane_task_dismissed.insert(pane_id.clone(), 1);

        state.refresh_task_progress();

        assert!(state.pane_task_progress.is_empty());
        assert!(state.pane_task_dismissed.is_empty());
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
        state.pane_task_dismissed.insert(pane_id.clone(), 1);
        let log_path = write_activity_log(
            &pane_id,
            "10:00|TaskCreate|#1 A\n10:01|TaskUpdate|in_progress #1\n",
        );

        state.refresh_task_progress();

        assert_eq!(state.pane_task_dismissed.get(&pane_id), None);
        assert_eq!(
            state.pane_task_progress.get(&pane_id).map(|p| p.total()),
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

        assert!(state.pane_task_progress.get(&pane_id).is_none());
        assert_eq!(state.pane_task_dismissed.get(&pane_id), Some(&1));
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
        assert_eq!(state.pane_task_dismissed.get(&pane_id), Some(&1));
        assert!(state.pane_task_progress.get(&pane_id).is_none());

        state.refresh_task_progress();
        assert_eq!(state.pane_task_dismissed.get(&pane_id), Some(&1));
        assert!(state.pane_task_progress.get(&pane_id).is_none());
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
        assert_eq!(state.pane_task_dismissed.get(&pane_id), Some(&1));

        // Pane removed — both dismissed and inactive_since should be cleaned up
        state.repo_groups.clear();
        state.pane_inactive_since.insert(pane_id.clone(), 100);
        state.refresh_task_progress();

        assert!(state.pane_task_dismissed.is_empty());
        assert!(state.pane_inactive_since.is_empty());
        fs::remove_file(&log_path).ok();
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
        assert!(state.pane_task_progress.get(&pane_id).is_some());
        assert!(state.pane_inactive_since.contains_key(&pane_id));

        // After grace period (3s): should be dismissed
        state.now = 104;
        state.refresh_task_progress();
        assert!(state.pane_task_progress.get(&pane_id).is_none());
        assert_eq!(state.pane_task_dismissed.get(&pane_id), Some(&6));
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
        assert!(state.pane_task_progress.get(&pane_id).is_some());
        assert_eq!(
            state.pane_task_progress.get(&pane_id).map(|p| p.total()),
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
        assert!(state.pane_task_progress.get(&pane_id).is_some());

        // After grace period: agent errored out — dismiss incomplete tasks
        state.now = 104;
        state.refresh_task_progress();
        assert!(state.pane_task_progress.get(&pane_id).is_none());
        assert_eq!(state.pane_task_dismissed.get(&pane_id), Some(&1));
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
        assert!(state.pane_task_progress.get(&pane_id).is_some());
        assert!(state.pane_inactive_since.contains_key(&pane_id));

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
        assert!(state.pane_task_progress.get(&pane_id).is_some());
        assert!(!state.pane_inactive_since.contains_key(&pane_id));

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
        assert!(state.pane_task_progress.get(&pane_id).is_some());

        // t=102 (2s elapsed): still within grace period — tasks shown
        state.now = 102;
        state.refresh_task_progress();
        assert!(state.pane_task_progress.get(&pane_id).is_some());

        // t=103 (exactly 3s): grace expired (>= 3) — dismissed
        state.now = 103;
        state.refresh_task_progress();
        assert!(state.pane_task_progress.get(&pane_id).is_none());
        assert_eq!(state.pane_task_dismissed.get(&pane_id), Some(&1));

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
        assert!(state.pane_task_progress.get(&pane_id).is_some());
        assert!(!state.pane_inactive_since.contains_key(&pane_id));

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
        state.global.selected_agent_row = 3;

        let pane = test_pane("%1");
        let sessions = vec![SessionInfo {
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
        assert_eq!(state.sessions.len(), 1);
        assert_eq!(state.repo_groups.len(), 1);
        assert_eq!(state.agent_row_targets.len(), 1);
        assert_eq!(state.global.selected_agent_row, 0);
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
        assert_eq!(state.agents_scroll.offset, 0);
    }

    #[test]
    fn mouse_scroll_in_agents_panel_scrolls_agents() {
        let mut state = AppState::new("%99".into());
        state.agents_scroll = ScrollState {
            offset: 0,
            total_lines: 40,
            visible_height: 20,
        };
        // term_height=50, bottom_panel=20 → bottom starts at row 30
        // mouse at row 10 → in agents panel
        state.handle_mouse_scroll(10, 50, 20, 3);
        assert_eq!(state.agents_scroll.offset, 3);
        assert_eq!(state.activity_scroll.offset, 0);
    }

    #[test]
    fn mouse_scroll_up_in_agents_panel() {
        let mut state = AppState::new("%99".into());
        state.agents_scroll = ScrollState {
            offset: 5,
            total_lines: 40,
            visible_height: 20,
        };
        state.handle_mouse_scroll(10, 50, 20, -3);
        assert_eq!(state.agents_scroll.offset, 2);
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
        assert_eq!(state.agents_scroll.offset, 0);
    }

    #[test]
    fn mouse_scroll_just_above_boundary_goes_to_agents() {
        let mut state = AppState::new("%99".into());
        state.agents_scroll = ScrollState {
            offset: 0,
            total_lines: 40,
            visible_height: 20,
        };
        // row 29, just above bottom_start=30
        state.handle_mouse_scroll(29, 50, 20, 3);
        assert_eq!(state.agents_scroll.offset, 3);
        assert_eq!(state.activity_scroll.offset, 0);
    }

    // ─── move_agent_selection edge cases ─────────────────────────────

    #[test]
    fn move_agent_selection_returns_false_when_empty() {
        let mut state = AppState::new("%99".into());
        assert!(!state.move_agent_selection(1));
        assert!(!state.move_agent_selection(-1));
    }

    #[test]
    fn move_agent_selection_boundary_returns() {
        let mut state = AppState::new("%99".into());
        state.agent_row_targets = vec![
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
        state.global.selected_agent_row = 0;

        assert!(!state.move_agent_selection(-1), "can't go below 0");
        assert!(state.move_agent_selection(1));
        assert!(state.move_agent_selection(1));
        assert_eq!(state.global.selected_agent_row, 2);
        assert!(!state.move_agent_selection(1), "can't go past end");
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
        state.global.selected_agent_row = 2;
        state.rebuild_row_targets();
        assert_eq!(state.global.selected_agent_row, 2);

        // Shrink to 1 pane
        state.repo_groups[0].panes = vec![(test_pane("%1"), PaneGitInfo::default())];
        state.rebuild_row_targets();
        assert_eq!(
            state.global.selected_agent_row, 0,
            "should clamp to last valid index"
        );
    }

    #[test]
    fn rebuild_row_targets_empty_groups() {
        let mut state = AppState::new("%99".into());
        state.global.selected_agent_row = 5;
        state.repo_groups = vec![];
        state.rebuild_row_targets();
        assert!(state.agent_row_targets.is_empty());
        // selected_agent_row stays as-is when targets empty (no clamp needed)
        assert_eq!(state.global.selected_agent_row, 5);
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
        state.global.agent_filter = AgentFilter::All;
        state.rebuild_row_targets();
        assert_eq!(state.agent_row_targets.len(), 3);

        // Running filter: only 2 panes
        state.global.agent_filter = AgentFilter::Running;
        state.rebuild_row_targets();
        assert_eq!(state.agent_row_targets.len(), 2);
        assert_eq!(state.agent_row_targets[0].pane_id, "%1");
        assert_eq!(state.agent_row_targets[1].pane_id, "%3");

        // Idle filter: only 1 pane
        state.global.agent_filter = AgentFilter::Idle;
        state.rebuild_row_targets();
        assert_eq!(state.agent_row_targets.len(), 1);
        assert_eq!(state.agent_row_targets[0].pane_id, "%2");

        // Error filter: no panes
        state.global.agent_filter = AgentFilter::Error;
        state.rebuild_row_targets();
        assert!(state.agent_row_targets.is_empty());
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
        state.global.agent_filter = AgentFilter::All;
        state.rebuild_row_targets();
        state.global.selected_agent_row = 2;

        // Switch to Running filter (only 1 pane) — cursor should clamp
        state.global.agent_filter = AgentFilter::Running;
        state.rebuild_row_targets();
        assert_eq!(state.global.selected_agent_row, 0);
        assert_eq!(state.agent_row_targets[0].pane_id, "%1");
    }

    // ─── handle_mouse_click tests ────────────────────────────────────

    #[test]
    fn mouse_click_selects_agent_row() {
        let mut state = AppState::new("%99".into());
        state.agent_row_targets = vec![
            RowTarget {
                pane_id: "%1".into(),
            },
            RowTarget {
                pane_id: "%2".into(),
            },
        ];
        // line_to_row: line 0 = group header (None), line 1 = agent 0, line 2 = agent 1
        state.line_to_row = vec![None, Some(0), Some(1)];
        state.agents_scroll.offset = 0;

        // row 0 = filter bar (skipped), row 1 = first agent list row
        state.handle_mouse_click(2, 5); // row 2 → line_index = (2-1) = 1 → agent row 0
        assert_eq!(state.global.selected_agent_row, 0);

        state.handle_mouse_click(3, 5); // row 3 → line_index = (3-1) = 2 → agent row 1
        assert_eq!(state.global.selected_agent_row, 1);
    }

    #[test]
    fn mouse_click_on_filter_bar_changes_filter() {
        let mut state = AppState::new("%99".into());
        state.agent_row_targets = vec![RowTarget {
            pane_id: "%1".into(),
        }];
        state.line_to_row = vec![None, Some(0)];
        state.global.selected_agent_row = 0;
        state.global.agent_filter = AgentFilter::All;

        // Click on "All" (x=1..3) should keep All
        reset_filter_debounce(&mut state);
        state.handle_mouse_click(0, 1);
        assert_eq!(state.global.agent_filter, AgentFilter::All);

        // Click on Running icon area (x=6..) should switch to Running
        reset_filter_debounce(&mut state);
        state.handle_mouse_click(0, 6);
        assert_eq!(state.global.agent_filter, AgentFilter::Running);

        // agent selection unchanged
        assert_eq!(state.global.selected_agent_row, 0);
    }

    #[test]
    fn mouse_click_with_scroll_offset() {
        let mut state = AppState::new("%99".into());
        state.agent_row_targets = vec![
            RowTarget {
                pane_id: "%1".into(),
            },
            RowTarget {
                pane_id: "%2".into(),
            },
        ];
        // 5 lines total, scrolled down by 2
        state.line_to_row = vec![None, Some(0), Some(0), None, Some(1)];
        state.agents_scroll.offset = 2;

        // row 3 → line_index = (3-1) + 2 = 4 → agent row 1
        state.handle_mouse_click(3, 5);
        assert_eq!(state.global.selected_agent_row, 1);
    }

    #[test]
    fn mouse_click_out_of_bounds() {
        let mut state = AppState::new("%99".into());
        state.agent_row_targets = vec![RowTarget {
            pane_id: "%1".into(),
        }];
        state.line_to_row = vec![None, Some(0)];
        state.global.selected_agent_row = 0;

        state.handle_mouse_click(50, 5); // way beyond line_to_row
        assert_eq!(state.global.selected_agent_row, 0); // unchanged
    }

    // ─── AgentFilter tests ───────────────────────────────────────────

    #[test]
    fn agent_filter_next_cycles() {
        assert_eq!(AgentFilter::All.next(), AgentFilter::Running);
        assert_eq!(AgentFilter::Running.next(), AgentFilter::Waiting);
        assert_eq!(AgentFilter::Waiting.next(), AgentFilter::Idle);
        assert_eq!(AgentFilter::Idle.next(), AgentFilter::Error);
        assert_eq!(AgentFilter::Error.next(), AgentFilter::All);
    }

    #[test]
    fn agent_filter_prev_cycles() {
        assert_eq!(AgentFilter::All.prev(), AgentFilter::Error);
        assert_eq!(AgentFilter::Error.prev(), AgentFilter::Idle);
        assert_eq!(AgentFilter::Idle.prev(), AgentFilter::Waiting);
        assert_eq!(AgentFilter::Waiting.prev(), AgentFilter::Running);
        assert_eq!(AgentFilter::Running.prev(), AgentFilter::All);
    }

    #[test]
    fn agent_filter_matches_status() {
        assert!(AgentFilter::All.matches(&PaneStatus::Running));
        assert!(AgentFilter::All.matches(&PaneStatus::Idle));
        assert!(AgentFilter::All.matches(&PaneStatus::Waiting));
        assert!(AgentFilter::All.matches(&PaneStatus::Error));

        assert!(AgentFilter::Running.matches(&PaneStatus::Running));
        assert!(!AgentFilter::Running.matches(&PaneStatus::Idle));
        assert!(!AgentFilter::Running.matches(&PaneStatus::Waiting));
        assert!(!AgentFilter::Running.matches(&PaneStatus::Error));

        assert!(AgentFilter::Waiting.matches(&PaneStatus::Waiting));
        assert!(!AgentFilter::Waiting.matches(&PaneStatus::Running));

        assert!(AgentFilter::Idle.matches(&PaneStatus::Idle));
        assert!(!AgentFilter::Idle.matches(&PaneStatus::Running));

        assert!(AgentFilter::Error.matches(&PaneStatus::Error));
        assert!(!AgentFilter::Error.matches(&PaneStatus::Idle));
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
        state.global.agent_filter = AgentFilter::Running;
        reset_filter_debounce(&mut state);
        state.handle_filter_click(1);
        assert_eq!(state.global.agent_filter, AgentFilter::All);

        reset_filter_debounce(&mut state);
        state.handle_filter_click(3);
        assert_eq!(state.global.agent_filter, AgentFilter::All);

        // "●0" at x=6..7
        reset_filter_debounce(&mut state);
        state.handle_filter_click(6);
        assert_eq!(state.global.agent_filter, AgentFilter::Running);

        // "◐0" at x=10..11
        reset_filter_debounce(&mut state);
        state.handle_filter_click(10);
        assert_eq!(state.global.agent_filter, AgentFilter::Waiting);

        // "○0" at x=14..15
        reset_filter_debounce(&mut state);
        state.handle_filter_click(14);
        assert_eq!(state.global.agent_filter, AgentFilter::Idle);

        // "✕0" at x=18..19
        reset_filter_debounce(&mut state);
        state.handle_filter_click(18);
        assert_eq!(state.global.agent_filter, AgentFilter::Error);
    }

    #[test]
    fn filter_click_gap_does_nothing() {
        let mut state = AppState::new("%99".into());
        state.global.agent_filter = AgentFilter::All;

        // x=0 is leading space, x=4 and x=5 are separator
        state.handle_filter_click(0);
        assert_eq!(state.global.agent_filter, AgentFilter::All);

        state.handle_filter_click(4);
        assert_eq!(state.global.agent_filter, AgentFilter::All);

        state.handle_filter_click(5);
        assert_eq!(state.global.agent_filter, AgentFilter::All);
    }

    #[test]
    fn filter_click_debounce_ignores_rapid_clicks() {
        let mut state = AppState::new("%99".into());
        state.global.agent_filter = AgentFilter::All;

        // First click within debounce window should be ignored
        // (AppState::new sets last_filter_click to now)
        state.handle_filter_click(6); // would be Running
        assert_eq!(state.global.agent_filter, AgentFilter::All); // unchanged due to debounce

        // After resetting debounce, click should work
        reset_filter_debounce(&mut state);
        state.handle_filter_click(6);
        assert_eq!(state.global.agent_filter, AgentFilter::Running);

        // Immediate second click should be debounced
        state.handle_filter_click(1); // would be All
        assert_eq!(state.global.agent_filter, AgentFilter::Running); // unchanged
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
        assert_eq!(state.global.agent_filter, AgentFilter::Running);
        reset_filter_debounce(&mut state);
        state.handle_filter_click(8);
        assert_eq!(state.global.agent_filter, AgentFilter::Running);

        // "◐0" shifts to x=11..12
        reset_filter_debounce(&mut state);
        state.handle_filter_click(11);
        assert_eq!(state.global.agent_filter, AgentFilter::Waiting);
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
        state.global.agent_filter = AgentFilter::All;
        state.rebuild_row_targets();
        assert_eq!(state.agent_row_targets.len(), 3);

        // Click Running filter — row_targets should update immediately
        // Layout: " All  ●2  ◐0  ○1  ✕0" → Running at x=6
        reset_filter_debounce(&mut state);
        state.handle_filter_click(6);
        assert_eq!(state.global.agent_filter, AgentFilter::Running);
        assert_eq!(state.agent_row_targets.len(), 2);
        assert_eq!(state.agent_row_targets[0].pane_id, "%1");
        assert_eq!(state.agent_row_targets[1].pane_id, "%3");

        // Click Idle filter — row_targets should update again
        // Layout: " All  ●2  ◐0  ○1  ✕0" → Idle at x=14
        reset_filter_debounce(&mut state);
        state.handle_filter_click(14);
        assert_eq!(state.global.agent_filter, AgentFilter::Idle);
        assert_eq!(state.agent_row_targets.len(), 1);
        assert_eq!(state.agent_row_targets[0].pane_id, "%2");
    }

    // ─── AgentFilter as_str / from_str tests ─────────────────────────

    #[test]
    fn agent_filter_as_str_all_variants() {
        assert_eq!(AgentFilter::All.as_str(), "all");
        assert_eq!(AgentFilter::Running.as_str(), "running");
        assert_eq!(AgentFilter::Waiting.as_str(), "waiting");
        assert_eq!(AgentFilter::Idle.as_str(), "idle");
        assert_eq!(AgentFilter::Error.as_str(), "error");
    }

    #[test]
    fn agent_filter_from_str_all_variants() {
        assert_eq!(AgentFilter::from_str("all"), AgentFilter::All);
        assert_eq!(AgentFilter::from_str("running"), AgentFilter::Running);
        assert_eq!(AgentFilter::from_str("waiting"), AgentFilter::Waiting);
        assert_eq!(AgentFilter::from_str("idle"), AgentFilter::Idle);
        assert_eq!(AgentFilter::from_str("error"), AgentFilter::Error);
    }

    #[test]
    fn agent_filter_from_str_unknown_defaults_to_all() {
        assert_eq!(AgentFilter::from_str(""), AgentFilter::All);
        assert_eq!(AgentFilter::from_str("unknown"), AgentFilter::All);
        assert_eq!(AgentFilter::from_str("Running"), AgentFilter::All); // case-sensitive
    }

    #[test]
    fn agent_filter_roundtrip() {
        for filter in AgentFilter::VARIANTS {
            assert_eq!(AgentFilter::from_str(filter.as_str()), filter);
        }
    }

    // ─── RepoFilter tests ─────────────────────────────────────

    #[test]
    fn repo_filter_persistence_roundtrip() {
        assert_eq!(RepoFilter::from_str("all"), RepoFilter::All);
        assert_eq!(RepoFilter::from_str(""), RepoFilter::All);
        assert_eq!(
            RepoFilter::from_str("my-app"),
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

        assert_eq!(state.agent_row_targets.len(), 2);
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

        assert_eq!(state.agent_row_targets.len(), 1);
        assert_eq!(state.agent_row_targets[0].pane_id, "%2");
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
        state.global.agent_filter = AgentFilter::Running;
        state.rebuild_row_targets();

        // Only Running panes in "app" group
        assert_eq!(state.agent_row_targets.len(), 1);
        assert_eq!(state.agent_row_targets[0].pane_id, "%1");
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
        assert_eq!(state.agent_row_targets.len(), 1);
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
        assert!(state.repo_popup_open);
        assert_eq!(state.repo_popup_selected, 0);

        // Close and set filter to "beta" → selected should be 2
        state.close_repo_popup();
        state.global.repo_filter = RepoFilter::Repo("beta".into());
        state.toggle_repo_popup();
        assert_eq!(state.repo_popup_selected, 2); // ["All", "alpha", "beta"]
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
        state.repo_popup_open = true;
        state.repo_popup_selected = 2; // "beta"
        state.confirm_repo_popup();

        assert_eq!(state.global.repo_filter, RepoFilter::Repo("beta".into()));
        assert!(!state.repo_popup_open);
        assert_eq!(state.agent_row_targets.len(), 1);
        assert_eq!(state.agent_row_targets[0].pane_id, "%2");
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
        state.repo_popup_open = true;
        state.repo_popup_selected = 0; // "All"
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

    #[test]
    fn cat_state_defaults() {
        let state = AppState::new("%0".into());
        assert!(matches!(state.cat_state, crate::ui::cat::CatState::Idle));
        assert_eq!(state.cat_x, crate::ui::cat::CAT_HOME_X);
        assert_eq!(state.cat_frame, 0);
        assert_eq!(state.cat_bob_timer, 0);
    }

    #[test]
    fn tick_cat_idle_to_walk_right_on_running() {
        let mut state = AppState::new("%0".into());
        let mut pane = test_pane("1");
        pane.status = PaneStatus::Running;
        state.repo_groups = vec![crate::group::RepoGroup {
            name: "repo".into(),
            has_focus: false,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        state.tick_cat(60);
        assert!(matches!(
            state.cat_state,
            crate::ui::cat::CatState::WalkRight
        ));
        assert!(state.cat_x > crate::ui::cat::CAT_HOME_X);
    }

    #[test]
    fn tick_cat_walk_right_to_working_at_desk() {
        let mut state = AppState::new("%0".into());
        let mut pane = test_pane("1");
        pane.status = PaneStatus::Running;
        state.repo_groups = vec![crate::group::RepoGroup {
            name: "repo".into(),
            has_focus: false,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        let panel_width = 60u16;
        let desk_x = panel_width.saturating_sub(
            crate::ui::cat::DESK_OFFSET + crate::ui::cat::DESK_WIDTH + crate::ui::cat::CAT_WIDTH,
        );
        state.cat_state = crate::ui::cat::CatState::WalkRight;
        state.cat_x = desk_x - 1;
        state.tick_cat(panel_width);
        assert!(matches!(state.cat_state, crate::ui::cat::CatState::Working));
    }

    #[test]
    fn tick_cat_working_to_walk_left_when_no_running() {
        let mut state = AppState::new("%0".into());
        let mut pane = test_pane("1");
        pane.status = PaneStatus::Idle;
        state.repo_groups = vec![crate::group::RepoGroup {
            name: "repo".into(),
            has_focus: false,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        state.cat_state = crate::ui::cat::CatState::Working;
        state.cat_x = 40;
        state.tick_cat(60);
        assert!(matches!(
            state.cat_state,
            crate::ui::cat::CatState::WalkLeft
        ));
    }

    #[test]
    fn tick_cat_walk_left_to_idle_at_home() {
        let mut state = AppState::new("%0".into());
        state.cat_state = crate::ui::cat::CatState::WalkLeft;
        state.cat_x = crate::ui::cat::CAT_HOME_X + 1;
        state.tick_cat(60);
        assert_eq!(state.cat_x, crate::ui::cat::CAT_HOME_X);
        state.tick_cat(60);
        assert!(matches!(state.cat_state, crate::ui::cat::CatState::Idle));
    }

    #[test]
    fn tick_cat_idle_bob() {
        let mut state = AppState::new("%0".into());
        for _ in 0..crate::ui::cat::BOB_INTERVAL {
            state.tick_cat(60);
        }
        assert_eq!(state.cat_bob_timer, 0);
    }
}
