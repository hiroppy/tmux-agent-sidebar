use vte::{Params, Parser, Perform};

#[derive(Debug, Clone, PartialEq)]
pub struct StyledCell {
    pub ch: char,
    pub fg: Option<u8>,
    pub bg: Option<u8>,
    pub bold: bool,
    pub reversed: bool,
    pub underlined: bool,
}

impl Default for StyledCell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: None,
            bg: None,
            bold: false,
            reversed: false,
            underlined: false,
        }
    }
}

pub fn parse_ansi(bytes: &[u8], cols: u16, rows: u16) -> Vec<Vec<StyledCell>> {
    let mut grid: Vec<Vec<StyledCell>> = (0..rows)
        .map(|_| vec![StyledCell::default(); cols as usize])
        .collect();
    let mut state = State {
        grid: &mut grid,
        cols,
        rows,
        cursor_x: 0,
        cursor_y: 0,
        style: StyledCell::default(),
    };
    let mut parser = Parser::new();
    parser.advance(&mut state, bytes);
    grid
}

struct State<'a> {
    grid: &'a mut Vec<Vec<StyledCell>>,
    cols: u16,
    rows: u16,
    cursor_x: u16,
    cursor_y: u16,
    style: StyledCell,
}

impl Perform for State<'_> {
    fn print(&mut self, ch: char) {
        if self.cursor_y < self.rows && self.cursor_x < self.cols {
            let mut cell = self.style.clone();
            cell.ch = ch;
            self.grid[self.cursor_y as usize][self.cursor_x as usize] = cell;
            self.cursor_x += 1;
        }
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => {
                if self.cursor_y + 1 < self.rows {
                    self.cursor_y += 1;
                }
                self.cursor_x = 0;
            }
            b'\r' => {
                self.cursor_x = 0;
            }
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        if action != 'm' {
            return;
        }
        let mut iter = params.iter();
        while let Some(p) = iter.next() {
            // Bare `CSI m` (empty param) is equivalent to `CSI 0 m` per
            // ECMA-48 — treat a missing code as 0 (reset) instead of
            // skipping, otherwise the previous cell's fg/bg/bold/etc.
            // leaks into the next one.
            let code = p.first().copied().unwrap_or(0);
            match code {
                0 => {
                    self.style = StyledCell {
                        ch: self.style.ch,
                        ..Default::default()
                    };
                }
                1 => self.style.bold = true,
                4 => self.style.underlined = true,
                7 => self.style.reversed = true,
                22 => self.style.bold = false,
                24 => self.style.underlined = false,
                27 => self.style.reversed = false,
                39 => self.style.fg = None,
                49 => self.style.bg = None,
                38 => {
                    if p.len() >= 3 && p[1] == 5 {
                        self.style.fg = Some(p[2] as u8);
                    } else if let (Some(mode), Some(color)) = (iter.next(), iter.next()) {
                        if !mode.is_empty() && mode[0] == 5 && !color.is_empty() {
                            self.style.fg = Some(color[0] as u8);
                        }
                    }
                }
                48 => {
                    if p.len() >= 3 && p[1] == 5 {
                        self.style.bg = Some(p[2] as u8);
                    } else if let (Some(mode), Some(color)) = (iter.next(), iter.next()) {
                        if !mode.is_empty() && mode[0] == 5 && !color.is_empty() {
                            self.style.bg = Some(color[0] as u8);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
