use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SessionNamesState {
    pub names: HashMap<String, String>,
}

impl SessionNamesState {
    pub fn new() -> Self {
        Self {
            names: HashMap::new(),
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
    fn new_starts_with_empty_map() {
        let state = SessionNamesState::new();
        assert!(state.names.is_empty());
    }

    #[test]
    fn default_delegates_to_new() {
        let default_state = SessionNamesState::default();
        assert!(default_state.names.is_empty());
    }
}
