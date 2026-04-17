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

#[derive(Debug, Clone, Default)]
pub struct ScrollStates {
    pub panes: ScrollState,
    pub git: ScrollState,
}
