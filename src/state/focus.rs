#[derive(Debug, Clone, PartialEq)]
pub enum Focus {
    Filter,
    Panes,
    ActivityLog,
}

impl Default for Focus {
    fn default() -> Self {
        Self::Panes
    }
}

#[derive(Debug, Clone)]
pub struct FocusState {
    pub sidebar_focused: bool,
    pub focus: Focus,
    pub focused_pane_id: Option<String>,
    pub prev_focused_pane_id: Option<String>,
}

impl FocusState {
    pub fn new() -> Self {
        Self {
            sidebar_focused: false,
            focus: Focus::Panes,
            focused_pane_id: None,
            prev_focused_pane_id: None,
        }
    }
}

impl Default for FocusState {
    fn default() -> Self {
        Self::new()
    }
}
