use crate::tmux;

use super::super::context::{append_subagent, drain_pending_teardowns, remove_subagent};

pub(in crate::cli::hook) fn on_subagent_start(
    pane: &str,
    agent_type: &str,
    agent_id: Option<&str>,
) -> i32 {
    // Claude Code always sends agent_id per the hooks spec; drop the
    // event silently if it's missing so the tree never gains an
    // untrackable entry.
    let Some(id) = agent_id.filter(|s| !s.is_empty()) else {
        return 0;
    };
    let current = tmux::get_pane_option_value(pane, "@pane_subagents");
    let new_val = append_subagent(&current, agent_type, id);
    tmux::set_pane_option(pane, "@pane_subagents", &new_val);
    0
}

pub(in crate::cli::hook) fn on_subagent_stop(pane: &str, agent_id: Option<&str>) -> i32 {
    let Some(id) = agent_id.filter(|s| !s.is_empty()) else {
        return 0;
    };
    let current = tmux::get_pane_option_value(pane, "@pane_subagents");
    let drained_to_empty = match remove_subagent(&current, id) {
        None => false,
        Some(new_val) if new_val.is_empty() => {
            tmux::unset_pane_option(pane, "@pane_subagents");
            true
        }
        Some(new_val) => {
            tmux::set_pane_option(pane, "@pane_subagents", &new_val);
            false
        }
    };
    // Once the last subagent stops, replay any teardown that was deferred
    // because subagents were active when SessionEnd / WorktreeRemove fired.
    if drained_to_empty {
        drain_pending_teardowns(pane);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::super::session::on_session_end;
    use super::super::worktree::on_worktree_remove;
    use super::*;
    use crate::cli::hook::context::{PENDING_SESSION_END, PENDING_WORKTREE_REMOVE};
    use crate::desktop_notification;
    use std::fs;

    fn default_notifications() -> desktop_notification::DesktopNotificationSettings {
        desktop_notification::DesktopNotificationSettings {
            enabled: false,
            events: Default::default(),
        }
    }

    #[test]
    fn on_subagent_start_appends_to_list() {
        let _guard = tmux::test_mock::install();
        let pane = "%SUB_START";
        on_subagent_start(pane, "Explore", Some("sub-1"));
        assert_eq!(
            tmux::test_mock::get(pane, "@pane_subagents").as_deref(),
            Some("Explore:sub-1")
        );
        on_subagent_start(pane, "Plan", Some("sub-2"));
        assert_eq!(
            tmux::test_mock::get(pane, "@pane_subagents").as_deref(),
            Some("Explore:sub-1,Plan:sub-2")
        );
    }

    #[test]
    fn on_subagent_start_drops_event_without_id() {
        let _guard = tmux::test_mock::install();
        let pane = "%SUB_NO_ID";
        on_subagent_start(pane, "Explore", None);
        assert!(!tmux::test_mock::contains(pane, "@pane_subagents"));
        on_subagent_start(pane, "Explore", Some(""));
        assert!(!tmux::test_mock::contains(pane, "@pane_subagents"));
    }

    // ─── deferred teardown regression tests ─────────────────────────
    //
    // These pin the invariant that SessionEnd / WorktreeRemove fired
    // while subagents are active must not be lost forever. They are
    // recorded as pending markers and replayed by on_subagent_stop once
    // the subagent list drains to empty.

    #[test]
    fn pending_session_end_drains_when_last_subagent_stops() {
        let _guard = tmux::test_mock::install();
        let pane = "%PARENT_DEFER";
        tmux::test_mock::set(pane, "@pane_subagents", "Explore:sub-1");
        tmux::test_mock::set(pane, "@pane_agent", "claude");
        tmux::test_mock::set(pane, "@pane_cwd", "/repo/parent");
        tmux::test_mock::set(pane, "@pane_status", "running");
        let log_path = crate::activity::log_file_path(pane);
        let _ = fs::create_dir_all(log_path.parent().unwrap());
        fs::write(&log_path, "1234567890|Read|main.rs\n").unwrap();

        // Parent SessionEnd arrives while a subagent is still running.
        on_session_end(pane, "claude", "", &default_notifications());
        assert!(
            tmux::test_mock::contains(pane, PENDING_SESSION_END),
            "SessionEnd must be deferred via the pending marker"
        );
        assert!(
            tmux::test_mock::contains(pane, "@pane_agent"),
            "deferred SessionEnd must not yet clear parent state"
        );
        assert!(log_path.exists(), "deferred SessionEnd must keep the log");

        // Last subagent stops — pending teardown should fire now.
        on_subagent_stop(pane, Some("sub-1"));

        assert!(
            !tmux::test_mock::contains(pane, "@pane_agent"),
            "drained SessionEnd should clear parent agent"
        );
        assert!(!tmux::test_mock::contains(pane, "@pane_cwd"));
        assert!(!tmux::test_mock::contains(pane, "@pane_status"));
        assert!(
            !tmux::test_mock::contains(pane, PENDING_SESSION_END),
            "pending marker must be cleared once teardown runs"
        );
        assert!(
            !log_path.exists(),
            "drained SessionEnd should remove the activity log"
        );
    }

    #[test]
    fn pending_worktree_remove_drains_when_last_subagent_stops() {
        let _guard = tmux::test_mock::install();
        let pane = "%PARENT_WT_DEFER";
        tmux::test_mock::set(pane, "@pane_subagents", "Explore:sub-1");
        tmux::test_mock::set(pane, "@pane_worktree_name", "feat");
        tmux::test_mock::set(pane, "@pane_worktree_branch", "feat");
        tmux::test_mock::set(pane, "@pane_cwd", "/wt/feat");

        on_worktree_remove(pane);
        assert!(
            tmux::test_mock::contains(pane, PENDING_WORKTREE_REMOVE),
            "WorktreeRemove must be deferred via the pending marker"
        );
        assert!(tmux::test_mock::contains(pane, "@pane_worktree_name"));

        on_subagent_stop(pane, Some("sub-1"));

        assert!(!tmux::test_mock::contains(pane, "@pane_worktree_name"));
        assert!(!tmux::test_mock::contains(pane, "@pane_worktree_branch"));
        assert!(!tmux::test_mock::contains(pane, "@pane_cwd"));
        assert!(
            !tmux::test_mock::contains(pane, PENDING_WORKTREE_REMOVE),
            "pending marker must be cleared once teardown runs"
        );
    }

    #[test]
    fn pending_teardown_does_not_fire_until_subagents_empty() {
        let _guard = tmux::test_mock::install();
        let pane = "%PARENT_PARTIAL";
        tmux::test_mock::set(pane, "@pane_subagents", "Explore:sub-1,Plan:sub-2");
        tmux::test_mock::set(pane, "@pane_agent", "claude");

        on_session_end(pane, "claude", "", &default_notifications());
        assert!(tmux::test_mock::contains(pane, PENDING_SESSION_END));

        // First child stops — list still has sub-2, teardown must NOT fire.
        on_subagent_stop(pane, Some("sub-1"));
        assert!(
            tmux::test_mock::contains(pane, "@pane_agent"),
            "teardown must wait for the LAST subagent"
        );
        assert!(tmux::test_mock::contains(pane, PENDING_SESSION_END));

        // Last child stops — now teardown fires.
        on_subagent_stop(pane, Some("sub-2"));
        assert!(!tmux::test_mock::contains(pane, "@pane_agent"));
        assert!(!tmux::test_mock::contains(pane, PENDING_SESSION_END));
    }
}
