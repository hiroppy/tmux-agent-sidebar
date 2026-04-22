use super::commands::run_tmux;

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
        set_pane_option("%1", "@pane_status", "running");
        assert_eq!(get_pane_option_value("%1", "@pane_status"), "running");
        assert!(test_mock::contains("%1", "@pane_status"));
        unset_pane_option("%1", "@pane_status");
        assert!(!test_mock::contains("%1", "@pane_status"));
        // `get` on missing key returns empty string (mock semantics).
        assert!(get_pane_option_value("%1", "@pane_status").is_empty());
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
