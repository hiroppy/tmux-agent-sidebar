use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SessionNamesState {
    pub names: HashMap<String, String>,
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
