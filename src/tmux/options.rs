use super::commands::run_tmux;

// ─── Pane-scoped option keys ─────────────────────────────────────────
//
// Single source of truth for every `@pane_*` tmux option the sidebar
// writes or reads. Hooks, the TUI refresh path, and the query layer
// all go through these constants so a typo can't silently corrupt
// pane state. Keep `clear_agent_pane_state` in `tmux/query.rs` and
// `clear_all_meta` in `cli/hook/context/meta.rs` in sync with this
// list — both sweep the full set on teardown.

/// Agent name the hooks identified for the pane (`claude` / `codex`
/// / `opencode`). Drives the sidebar's per-row icon.
pub const PANE_AGENT: &str = "@pane_agent";
/// Visual attention flag (`notification` / `clear`) that lights up
/// the row when a hook wants the user's eye.
pub const PANE_ATTENTION: &str = "@pane_attention";
/// Hook-reported working directory, preferred over tmux's
/// `pane_current_path` for repo grouping.
pub const PANE_CWD: &str = "@pane_cwd";
/// Latest backgrounded Bash command (sanitized). Presence is the
/// authoritative "live shell" signal: Stop routes to `background`
/// while this is set, and the sidebar surfaces the command text.
pub const PANE_BG_CMD: &str = "@pane_bg_cmd";
/// Value written to [`PANE_BG_CMD`] when the hook payload omits the real
/// command. The ps liveness sweep matches on this to skip its own entries
/// (placeholder has no process to verify against).
pub const BG_CMD_PLACEHOLDER: &str = "(background shell)";
/// Epoch-ms identifier regenerated on every SessionStart so
/// notification fingerprints stay scoped to the current run and
/// don't dedupe across restarts.
pub const PANE_NOTIFICATION_RUN_ID: &str = "@pane_notification_run_id";
/// Last fingerprint we fired a `PermissionRequired` desktop
/// notification for — used to suppress duplicates.
pub const PANE_OS_NOTIFY_PERMISSION_REQUIRED: &str = "@pane_os_notify_permission_required";
/// Same dedup stamp for `TaskCompleted` notifications.
pub const PANE_OS_NOTIFY_TASK_COMPLETED: &str = "@pane_os_notify_task_completed";
/// Same dedup stamp for `TaskFailed` notifications.
pub const PANE_OS_NOTIFY_TASK_FAILED: &str = "@pane_os_notify_task_failed";
/// Legacy marker — see
/// `cli/hook/context/pending.rs::PENDING_SESSION_END` for the
/// rationale for keeping it defined but never set.
pub const PANE_PENDING_SESSION_END: &str = "@pane_pending_session_end";
/// Pending WorktreeRemove marker drained by `on_subagent_stop`
/// when the last subagent exits.
pub const PANE_PENDING_WORKTREE_REMOVE: &str = "@pane_pending_worktree_remove";
/// Permission mode in use by the agent (e.g. `plan`,
/// `acceptEdits`, `bypassPermissions`).
pub const PANE_PERMISSION_MODE: &str = "@pane_permission_mode";
/// Last user prompt the agent received. Shown in the bottom tab
/// as activity context.
pub const PANE_PROMPT: &str = "@pane_prompt";
/// Where the prompt came from (e.g. `UserPromptSubmit` vs
/// resumed session) — drives rendering nuance.
pub const PANE_PROMPT_SOURCE: &str = "@pane_prompt_source";
/// Pane role marker set by the setup flow (`sidebar` for the
/// sidebar pane itself) so the TUI can exclude itself from the
/// agent list.
pub const PANE_ROLE: &str = "@pane_role";
/// Agent-provided session id, surfaced in the status line for
/// quick reference.
pub const PANE_SESSION_ID: &str = "@pane_session_id";
/// Epoch-seconds timestamp of the current run's start — drives
/// the "running for Xs" label.
pub const PANE_STARTED_AT: &str = "@pane_started_at";
/// High-level status (`idle` / `running` / `waiting` / `clear`).
pub const PANE_STATUS: &str = "@pane_status";
/// Comma-separated `Type:id` list of currently-active subagents.
/// Non-empty ⇒ the pane is hosting subagent events and writes
/// from their hooks must be filtered out of parent metadata.
pub const PANE_SUBAGENTS: &str = "@pane_subagents";
/// Reason the pane is in `waiting` status (`permission`,
/// `session_resumed`, etc.).
pub const PANE_WAIT_REASON: &str = "@pane_wait_reason";
/// Branch name when the pane is attached to a git worktree.
pub const PANE_WORKTREE_BRANCH: &str = "@pane_worktree_branch";
/// Worktree slug (directory basename) for the attached worktree.
pub const PANE_WORKTREE_NAME: &str = "@pane_worktree_name";

pub fn get_option(name: &str) -> Option<String> {
    run_tmux(&["show", "-gv", name])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Fetch all global tmux options in a single subprocess call.
/// Returns a map of option name → value.
pub fn get_all_global_options() -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    if let Some(output) = run_tmux(&["show", "-g"]) {
        for line in output.lines() {
            // Format: "option-name value" or "@user_option value"
            if let Some((key, value)) = line.split_once(' ') {
                map.insert(key.to_string(), value.trim_matches('"').to_string());
            }
        }
    }
    map
}

pub fn set_pane_option(pane: &str, key: &str, value: &str) {
    #[cfg(test)]
    if test_mock::intercept_set(pane, key, value) {
        return;
    }
    let _ = run_tmux(&["set", "-t", pane, "-p", key, value]);
}

pub fn unset_pane_option(pane: &str, key: &str) {
    #[cfg(test)]
    if test_mock::intercept_unset(pane, key) {
        return;
    }
    let _ = run_tmux(&["set", "-t", pane, "-p", "-u", key]);
}

pub fn get_pane_option_value(pane: &str, key: &str) -> String {
    #[cfg(test)]
    if let Some(value) = test_mock::intercept_get(pane, key) {
        return value;
    }
    run_tmux(&["show", "-t", pane, "-pv", key])
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Per-thread in-memory tmux pane store used by tests. Activated by
/// installing a mock with [`test_mock::install`]; until then, all
/// `set/unset/get_pane_option*` calls fall through to the real `tmux`
/// command. The whole module is `cfg(test)` so it has zero cost in
/// release builds.
#[cfg(test)]
pub mod test_mock {
    use std::cell::RefCell;
    use std::collections::HashMap;

    type Store = HashMap<(String, String), String>;

    thread_local! {
        static MOCK: RefCell<Option<Store>> = const { RefCell::new(None) };
    }

    /// Install a fresh mock store for the current thread. Returns a guard
    /// that uninstalls the mock on drop so concurrent tests don't leak
    /// state across each other.
    pub fn install() -> MockGuard {
        MOCK.with(|m| *m.borrow_mut() = Some(Store::new()));
        MockGuard
    }

    pub struct MockGuard;

    impl Drop for MockGuard {
        fn drop(&mut self) {
            MOCK.with(|m| *m.borrow_mut() = None);
        }
    }

    /// Pre-populate a pane option in the mock store. Call after `install`.
    pub fn set(pane: &str, key: &str, value: &str) {
        MOCK.with(|m| {
            if let Some(store) = m.borrow_mut().as_mut() {
                store.insert((pane.to_string(), key.to_string()), value.to_string());
            }
        });
    }

    /// Read a pane option from the mock store. Returns `None` if no mock
    /// is installed (so production code paths still hit real tmux).
    pub fn get(pane: &str, key: &str) -> Option<String> {
        MOCK.with(|m| {
            m.borrow().as_ref().map(|store| {
                store
                    .get(&(pane.to_string(), key.to_string()))
                    .cloned()
                    .unwrap_or_default()
            })
        })
    }

    /// Returns true if a key exists in the mock store. Useful for
    /// asserting that a teardown DID NOT remove a key.
    pub fn contains(pane: &str, key: &str) -> bool {
        MOCK.with(|m| {
            m.borrow()
                .as_ref()
                .is_some_and(|store| store.contains_key(&(pane.to_string(), key.to_string())))
        })
    }

    pub(super) fn intercept_set(pane: &str, key: &str, value: &str) -> bool {
        MOCK.with(|m| {
            if let Some(store) = m.borrow_mut().as_mut() {
                store.insert((pane.to_string(), key.to_string()), value.to_string());
                true
            } else {
                false
            }
        })
    }

    pub(super) fn intercept_unset(pane: &str, key: &str) -> bool {
        MOCK.with(|m| {
            if let Some(store) = m.borrow_mut().as_mut() {
                store.remove(&(pane.to_string(), key.to_string()));
                true
            } else {
                false
            }
        })
    }

    pub(super) fn intercept_get(pane: &str, key: &str) -> Option<String> {
        MOCK.with(|m| {
            m.borrow().as_ref().map(|store| {
                store
                    .get(&(pane.to_string(), key.to_string()))
                    .cloned()
                    .unwrap_or_default()
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_install_round_trips_pane_option() {
        let _guard = test_mock::install();
        set_pane_option("%1", PANE_STATUS, "running");
        assert_eq!(get_pane_option_value("%1", PANE_STATUS), "running");
        assert!(test_mock::contains("%1", PANE_STATUS));
        unset_pane_option("%1", PANE_STATUS);
        assert!(!test_mock::contains("%1", PANE_STATUS));
        // `get` on missing key returns empty string (mock semantics).
        assert!(get_pane_option_value("%1", PANE_STATUS).is_empty());
    }

    #[test]
    fn mock_helpers_get_and_contains_when_installed() {
        let _guard = test_mock::install();
        test_mock::set("%9", "@foo", "bar");
        assert_eq!(test_mock::get("%9", "@foo").as_deref(), Some("bar"));
        assert_eq!(test_mock::get("%9", "@missing").as_deref(), Some(""));
    }

    #[test]
    fn mock_guard_uninstalls_on_drop() {
        {
            let _guard = test_mock::install();
            test_mock::set("%7", "@x", "y");
            assert!(test_mock::contains("%7", "@x"));
        }
        // No mock installed now — `contains` returns false.
        assert!(!test_mock::contains("%7", "@x"));
    }
}
