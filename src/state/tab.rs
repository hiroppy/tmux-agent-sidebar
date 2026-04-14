use super::{AppState, BottomTab};

impl AppState {
    /// Auto-switch bottom tab based on the focused pane.
    ///
    /// - Focus changed → save old pane's tab, restore new pane's tab
    /// - New agent pane (first seen) → Activity tab (once only)
    /// - Non-agent pane with no saved pref → Git tab
    pub(crate) fn auto_switch_tab(&mut self) {
        let focus_changed = self.focused_pane_id != self.prev_focused_pane_id;
        if focus_changed {
            self.save_current_tab();
        }
        // detect_new_agents cleans up disappeared agents (removing their
        // seen status and saved tab prefs), so it must run after save
        // to avoid re-saving a stale pref for a closed agent.
        let new_agent_ids = self.detect_new_agents();

        match self.resolve_bottom_tab(focus_changed, &new_agent_ids) {
            TabDecision::Keep => {}
            TabDecision::Set(tab) => {
                self.bottom_tab = tab;
            }
        }

        if focus_changed {
            self.prev_focused_pane_id = self.focused_pane_id.clone();
        }
    }

    /// Decide what the bottom tab should do for the current focus state.
    ///
    /// `Keep` means "leave the current tab as-is".
    /// `Set(tab)` means "switch to the given tab now".
    fn resolve_bottom_tab(
        &self,
        focus_changed: bool,
        new_agent_pane_ids: &std::collections::HashSet<String>,
    ) -> TabDecision {
        if focus_changed {
            return self.resolve_tab_for_focused_pane(new_agent_pane_ids);
        }

        if let Some(ref fid) = self.focused_pane_id
            && new_agent_pane_ids.contains(fid)
        {
            // Agent started in the currently focused pane.
            return TabDecision::Set(BottomTab::Activity);
        }

        TabDecision::Keep
    }

    /// Register all current agent panes. Returns IDs of newly appeared agents.
    /// Also removes agents that have disappeared so that re-launching
    /// an agent in the same pane is detected as new.
    fn detect_new_agents(&mut self) -> std::collections::HashSet<String> {
        let mut current: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut new_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        for group in &self.repo_groups {
            for (pane, _) in &group.panes {
                current.insert(pane.pane_id.clone());
                if self.seen_agent_panes.insert(pane.pane_id.clone()) {
                    new_ids.insert(pane.pane_id.clone());
                }
            }
        }
        // Remove disappeared agents from seen set so that relaunching an
        // agent is detected as new. Also clear the matching `tab_pref` so a
        // relaunched pane starts on the default tab — explicit here because
        // detect_new_agents can run before `prune_pane_states_to_current_panes`
        // (e.g. when tests mutate `repo_groups` directly without going
        // through `apply_session_snapshot`).
        let removed: Vec<String> = self
            .seen_agent_panes
            .iter()
            .filter(|id| !current.contains(id.as_str()))
            .cloned()
            .collect();
        for id in &removed {
            self.seen_agent_panes.remove(id);
            if let Some(state) = self.pane_states.get_mut(id) {
                state.tab_pref = None;
            }
        }
        new_ids
    }

    /// Save the current tab preference for the pane we're leaving.
    fn save_current_tab(&mut self) {
        if let Some(prev_id) = self.prev_focused_pane_id.clone() {
            let tab = self.bottom_tab.clone();
            self.pane_state_mut(&prev_id).tab_pref = Some(tab);
        }
    }

    /// Restore the saved tab for the pane we're entering,
    /// or pick a sensible default.
    fn resolve_tab_for_focused_pane(
        &self,
        new_agent_pane_ids: &std::collections::HashSet<String>,
    ) -> TabDecision {
        let Some(ref cur_id) = self.focused_pane_id else {
            return TabDecision::Keep;
        };
        if let Some(saved) = self.pane_state(cur_id).and_then(|s| s.tab_pref.as_ref()) {
            TabDecision::Set(saved.clone())
        } else if new_agent_pane_ids.contains(cur_id) || self.focused_pane_is_agent() {
            // The focused pane is an agent, and there's no saved preference yet.
            TabDecision::Set(BottomTab::Activity)
        } else {
            TabDecision::Set(BottomTab::GitStatus)
        }
    }

    /// Check if the focused pane is an agent pane (present in repo_groups).
    pub(crate) fn focused_pane_is_agent(&self) -> bool {
        let Some(ref fid) = self.focused_pane_id else {
            return false;
        };
        self.repo_groups
            .iter()
            .any(|g| g.panes.iter().any(|(p, _)| p.pane_id == *fid))
    }
}

#[derive(Debug, Clone, PartialEq)]
enum TabDecision {
    Keep,
    Set(BottomTab),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::{PaneGitInfo, RepoGroup};
    use crate::tmux::{AgentType, PaneInfo, PaneStatus, PermissionMode};

    fn test_pane(id: &str) -> PaneInfo {
        PaneInfo {
            pane_id: id.into(),
            pane_active: true,
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

    fn agent_group(pane_id: &str) -> RepoGroup {
        RepoGroup {
            name: "project".into(),
            has_focus: true,
            panes: vec![(test_pane(pane_id), PaneGitInfo::default())],
        }
    }

    fn state_with_groups(repo_groups: Vec<RepoGroup>, focused_pane_id: Option<&str>) -> AppState {
        let mut state = AppState::new("%99".into());
        state.repo_groups = repo_groups;
        state.focused_pane_id = focused_pane_id.map(str::to_string);
        state
    }

    // ─── focused_pane_is_agent ───────────────────────────────────

    #[test]
    fn focused_pane_is_agent_true() {
        let mut state = AppState::new("%99".into());
        state.focused_pane_id = Some("%1".into());
        state.repo_groups = vec![agent_group("%1")];
        assert!(state.focused_pane_is_agent());
    }

    #[test]
    fn focused_pane_is_agent_false_non_agent() {
        let mut state = AppState::new("%99".into());
        state.focused_pane_id = Some("%5".into());
        state.repo_groups = vec![agent_group("%1")];
        assert!(!state.focused_pane_is_agent());
    }

    #[test]
    fn focused_pane_is_agent_false_no_focus() {
        let mut state = AppState::new("%99".into());
        state.focused_pane_id = None;
        state.repo_groups = vec![agent_group("%1")];
        assert!(!state.focused_pane_is_agent());
    }

    #[test]
    fn focused_pane_is_agent_false_empty_groups() {
        let mut state = AppState::new("%99".into());
        state.focused_pane_id = Some("%1".into());
        state.repo_groups = vec![];
        assert!(!state.focused_pane_is_agent());
    }

    // ─── detect_new_agents ──────────────────────────────────────

    #[test]
    fn detect_new_agents_empty() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![];
        assert!(state.detect_new_agents().is_empty());
    }

    #[test]
    fn detect_new_agents_first_time() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![agent_group("%1")];
        let new_ids = state.detect_new_agents();
        assert!(new_ids.contains("%1"));
        assert!(state.seen_agent_panes.contains("%1"));
    }

    #[test]
    fn detect_new_agents_already_seen() {
        let mut state = AppState::new("%99".into());
        state.seen_agent_panes.insert("%1".into());
        state.repo_groups = vec![agent_group("%1")];
        assert!(state.detect_new_agents().is_empty());
    }

    // ─── scenario: full lifecycle ───────────────────────────────

    #[test]
    fn scenario_full_lifecycle() {
        let mut state = state_with_groups(vec![], Some("%5"));

        // Step 1: Sidebar starts, focus on non-agent pane %5
        state.auto_switch_tab();
        assert_eq!(
            state.bottom_tab,
            BottomTab::GitStatus,
            "step 1: non-agent → Git"
        );

        // Step 2: Agent %1 starts, focus moves to it
        state.repo_groups = vec![agent_group("%1")];
        state.focused_pane_id = Some("%1".into());
        state.auto_switch_tab();
        assert_eq!(
            state.bottom_tab,
            BottomTab::Activity,
            "step 2: new agent → Activity"
        );

        // Step 3: Subsequent refresh (no focus change) → no change
        state.auto_switch_tab();
        assert_eq!(
            state.bottom_tab,
            BottomTab::Activity,
            "step 3: same focus → no change"
        );

        // Step 4: User manually switches to Git
        state.next_bottom_tab();
        state.auto_switch_tab();
        assert_eq!(
            state.bottom_tab,
            BottomTab::GitStatus,
            "step 4: manual Git → respected"
        );

        // Step 5: Focus to non-agent %5
        state.focused_pane_id = Some("%5".into());
        state.auto_switch_tab();
        assert_eq!(
            state.bottom_tab,
            BottomTab::GitStatus,
            "step 5: non-agent → Git"
        );

        // Step 6: Focus back to %1 → restores saved Git pref
        state.focused_pane_id = Some("%1".into());
        state.auto_switch_tab();
        assert_eq!(
            state.bottom_tab,
            BottomTab::GitStatus,
            "step 6: restore %1's Git pref"
        );
    }

    // ─── scenario: per-pane tab memory ──────────────────────────

    #[test]
    fn scenario_per_pane_tab_memory() {
        let mut state = state_with_groups(vec![agent_group("%1")], Some("%1"));

        // Agent %1 → Activity
        state.auto_switch_tab();
        assert_eq!(state.bottom_tab, BottomTab::Activity);

        // User switches %1 to Git
        state.next_bottom_tab();

        // Agent %2 starts, focus moves to %2
        let mut group = agent_group("%1");
        group.panes.push((test_pane("%2"), PaneGitInfo::default()));
        state.repo_groups = vec![group];
        state.focused_pane_id = Some("%2".into());
        state.auto_switch_tab();
        assert_eq!(
            state.bottom_tab,
            BottomTab::Activity,
            "%2: new agent → Activity"
        );

        // Focus back to %1 → Git (saved)
        state.focused_pane_id = Some("%1".into());
        state.auto_switch_tab();
        assert_eq!(state.bottom_tab, BottomTab::GitStatus, "%1: restored Git");

        // Focus back to %2 → Activity (saved)
        state.focused_pane_id = Some("%2".into());
        state.auto_switch_tab();
        assert_eq!(
            state.bottom_tab,
            BottomTab::Activity,
            "%2: restored Activity"
        );
    }

    // ─── scenario: manual tab preserved across refreshes ────────

    #[test]
    fn scenario_manual_tab_preserved_across_refreshes() {
        let mut state = state_with_groups(vec![agent_group("%1")], Some("%1"));

        state.auto_switch_tab();

        // User switches to Git
        state.next_bottom_tab();

        // 5 refreshes (no focus change)
        for _ in 0..5 {
            state.auto_switch_tab();
        }
        assert_eq!(
            state.bottom_tab,
            BottomTab::GitStatus,
            "manual Git survives refreshes"
        );
    }

    // ─── scenario: new agent in same pane ───────────────────────

    #[test]
    fn scenario_new_agent_in_same_pane() {
        let mut state = state_with_groups(vec![], Some("%1"));

        // Focus on %1, no agent
        state.auto_switch_tab();
        assert_eq!(state.bottom_tab, BottomTab::GitStatus);

        // Agent starts in %1 (no focus change)
        state.repo_groups = vec![agent_group("%1")];
        state.auto_switch_tab();
        assert_eq!(
            state.bottom_tab,
            BottomTab::Activity,
            "new agent without focus change"
        );
    }

    #[test]
    fn scenario_new_agent_elsewhere_does_not_override_existing_tab() {
        let mut state = state_with_groups(vec![agent_group("%1")], Some("%5"));
        state.auto_switch_tab();
        assert_eq!(state.bottom_tab, BottomTab::GitStatus);

        state.next_bottom_tab();
        assert_eq!(state.bottom_tab, BottomTab::Activity);

        // New agent appears in a different pane, but focus stays on the shell.
        let mut group = agent_group("%1");
        group.panes.push((test_pane("%2"), PaneGitInfo::default()));
        state.repo_groups = vec![group];
        state.auto_switch_tab();

        assert_eq!(
            state.bottom_tab,
            BottomTab::Activity,
            "new agent elsewhere should not force a tab change"
        );
    }

    // ─── scenario: focus to existing agent (no saved pref) ──────

    #[test]
    fn scenario_focus_to_existing_agent_defaults_activity() {
        let mut state = state_with_groups(vec![agent_group("%1")], Some("%5"));

        // %1 agent already seen, currently on non-agent %5
        state.seen_agent_panes.insert("%1".into());
        state.prev_focused_pane_id = Some("%5".into());
        state.bottom_tab = BottomTab::GitStatus;

        // Focus to %1 (no saved pref for %1)
        state.focused_pane_id = Some("%1".into());
        state.auto_switch_tab();
        assert_eq!(
            state.bottom_tab,
            BottomTab::Activity,
            "existing agent with no saved pref → Activity"
        );
    }

    // ─── scenario: focus changes to None ────────────────────────

    #[test]
    fn scenario_focus_becomes_none() {
        let mut state = state_with_groups(vec![agent_group("%1")], Some("%1"));

        state.auto_switch_tab();
        assert_eq!(state.bottom_tab, BottomTab::Activity);

        // Focus becomes None (all panes closed?)
        state.focused_pane_id = None;
        state.auto_switch_tab();
        // restore_or_default_tab returns early for None, so tab stays
        assert_eq!(
            state.bottom_tab,
            BottomTab::Activity,
            "None focus → tab unchanged"
        );
    }

    #[test]
    fn scenario_focus_none_then_returns_to_same_pane_preserves_tab() {
        let mut state = state_with_groups(vec![agent_group("%1")], Some("%1"));

        state.auto_switch_tab();
        assert_eq!(state.bottom_tab, BottomTab::Activity);

        state.next_bottom_tab();
        assert_eq!(state.bottom_tab, BottomTab::GitStatus);

        state.focused_pane_id = None;
        state.auto_switch_tab();
        assert_eq!(state.bottom_tab, BottomTab::GitStatus);

        state.focused_pane_id = Some("%1".into());
        state.auto_switch_tab();
        assert_eq!(
            state.bottom_tab,
            BottomTab::GitStatus,
            "returning to the same pane after None should preserve the tab"
        );
    }

    #[test]
    fn scenario_restore_saved_tab_when_returning_to_pane() {
        let mut state = state_with_groups(vec![agent_group("%1")], Some("%1"));

        state.auto_switch_tab();
        assert_eq!(state.bottom_tab, BottomTab::Activity);

        state.next_bottom_tab();
        assert_eq!(state.bottom_tab, BottomTab::GitStatus);

        state.focused_pane_id = Some("%5".into());
        state.repo_groups = vec![agent_group("%1")];
        state.auto_switch_tab();
        assert_eq!(state.bottom_tab, BottomTab::GitStatus);

        state.focused_pane_id = Some("%1".into());
        state.repo_groups = vec![agent_group("%1")];
        state.auto_switch_tab();
        assert_eq!(
            state.bottom_tab,
            BottomTab::GitStatus,
            "saved tab should be restored when focus returns"
        );
    }

    // ─── scenario: non-agent pane with other agent present ─────

    #[test]
    fn scenario_startup_non_agent_focus_with_other_agent() {
        // Sidebar starts, focus is on a non-agent pane but another
        // pane has an agent. The focused pane should get Git, not Activity.
        let mut state = state_with_groups(vec![agent_group("%1")], Some("%5"));
        state.auto_switch_tab();
        assert_eq!(
            state.bottom_tab,
            BottomTab::GitStatus,
            "non-agent pane should get Git even when other agents exist"
        );
    }

    #[test]
    fn scenario_focus_to_non_agent_while_agent_starts_elsewhere() {
        let mut state = state_with_groups(vec![], Some("%5"));

        // Start on %5 (shell)
        state.auto_switch_tab();
        assert_eq!(state.bottom_tab, BottomTab::GitStatus);

        // Agent %1 starts in another pane, but focus stays on %5
        state.repo_groups = vec![agent_group("%1")];
        // no focus change
        state.auto_switch_tab();
        assert_eq!(
            state.bottom_tab,
            BottomTab::GitStatus,
            "should stay on Git when new agent is in a different pane"
        );
    }

    // ─── scenario: agent disappears then reappears ──────────────

    #[test]
    fn scenario_agent_closes_and_relaunches() {
        let mut state = state_with_groups(vec![agent_group("%1")], Some("%1"));

        // Agent %1 starts
        state.auto_switch_tab();
        assert_eq!(state.bottom_tab, BottomTab::Activity);

        // User switches to Git
        state.next_bottom_tab();
        assert_eq!(state.bottom_tab, BottomTab::GitStatus);

        // Agent %1 closes (disappears from repo_groups)
        state.repo_groups = vec![];
        state.focused_pane_id = Some("%5".into());
        state.auto_switch_tab();
        // %1 should be removed from seen_agent_panes
        assert!(
            !state.seen_agent_panes.contains("%1"),
            "closed agent should be removed from seen set"
        );

        // Agent relaunches in same pane %1
        state.repo_groups = vec![agent_group("%1")];
        state.focused_pane_id = Some("%1".into());
        state.auto_switch_tab();
        assert_eq!(
            state.bottom_tab,
            BottomTab::Activity,
            "relaunched agent should trigger Activity"
        );
    }
}
