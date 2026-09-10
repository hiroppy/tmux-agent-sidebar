use std::collections::HashMap;

use crate::session::SessionMeta;

#[derive(Debug, Clone)]
pub struct SessionNamesState {
    /// `sessionId → metadata` (name + start time), refreshed off-thread from
    /// `~/.claude/sessions/*.json`.
    pub names: HashMap<String, SessionMeta>,
    pub dirty: bool,
}

impl SessionNamesState {
    pub fn new() -> Self {
        Self {
            names: HashMap::new(),
            dirty: true,
        }
    }
}

impl Default for SessionNamesState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_dirty_with_empty_map() {
        let state = SessionNamesState::new();
        assert!(
            state.dirty,
            "fresh state should be dirty so the first refresh runs"
        );
        assert!(state.names.is_empty());
    }

    #[test]
    fn default_delegates_to_new() {
        let default_state = SessionNamesState::default();
        assert!(default_state.dirty);
        assert!(default_state.names.is_empty());
    }
}
