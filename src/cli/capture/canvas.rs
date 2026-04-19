use crate::cli::capture::ansi::StyledCell;
use crate::cli::capture::tmux_probe::PaneGeom;

#[derive(Debug, Clone, Copy)]
pub struct WindowGeom {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone)]
pub struct PaneContent {
    pub geom: PaneGeom,
    /// `geom.height` rows × `geom.width` cells.
    pub cells: Vec<Vec<StyledCell>>,
}

/// Assemble a window-sized grid from painted panes + drawn pane borders.
pub fn assemble(win: &WindowGeom, panes: &[PaneContent]) -> Vec<Vec<StyledCell>> {
    let mut canvas: Vec<Vec<StyledCell>> = (0..win.rows)
        .map(|_| vec![StyledCell::default(); win.cols as usize])
        .collect();

    paint_panes(&mut canvas, win, panes);
    draw_vertical_dividers(&mut canvas, win, panes);
    draw_horizontal_dividers(&mut canvas, win, panes);
    resolve_junctions(&mut canvas, win);

    canvas
}

fn paint_panes(canvas: &mut [Vec<StyledCell>], win: &WindowGeom, panes: &[PaneContent]) {
    for pane in panes {
        for (row_idx, row) in pane.cells.iter().enumerate() {
            let y = pane.geom.top as usize + row_idx;
            if y >= win.rows as usize {
                break;
            }
            for (col_idx, cell) in row.iter().enumerate() {
                let x = pane.geom.left as usize + col_idx;
                if x >= win.cols as usize {
                    break;
                }
                canvas[y][x] = cell.clone();
            }
        }
    }
}

fn draw_vertical_dividers(canvas: &mut [Vec<StyledCell>], win: &WindowGeom, panes: &[PaneContent]) {
    for pane in panes {
        if pane.geom.left == 0 {
            continue; // no divider to the left of a pane at x=0
        }
        let div_col = (pane.geom.left - 1) as usize;
        if div_col >= win.cols as usize {
            continue;
        }
        let top = (pane.geom.top as usize).min(win.rows as usize);
        let bottom =
            ((pane.geom.top as usize) + (pane.geom.height as usize)).min(win.rows as usize);
        for row in canvas[top..bottom].iter_mut() {
            if div_col < row.len() && is_blank(&row[div_col]) {
                row[div_col] = divider_cell('│');
            }
        }
    }
}

fn draw_horizontal_dividers(
    canvas: &mut [Vec<StyledCell>],
    win: &WindowGeom,
    panes: &[PaneContent],
) {
    for pane in panes {
        if pane.geom.top == 0 {
            continue;
        }
        let div_row = (pane.geom.top - 1) as usize;
        if div_row >= canvas.len() {
            continue;
        }
        let left = (pane.geom.left as usize).min(win.cols as usize);
        let right = ((pane.geom.left as usize) + (pane.geom.width as usize)).min(win.cols as usize);
        let row = &mut canvas[div_row];
        let left = left.min(row.len());
        let right = right.min(row.len());
        for cell in row[left..right].iter_mut() {
            if is_blank(cell) {
                *cell = divider_cell('─');
            }
        }
    }
}

fn resolve_junctions(canvas: &mut [Vec<StyledCell>], win: &WindowGeom) {
    // For each cell, look at the four neighbours:
    // - If the cell already carries a divider glyph we drew, upgrade it to the
    //   correct corner/T-junction/cross based on which neighbours are also
    //   *our* dividers.
    // - If the cell is blank but has our-divider neighbours on both axes,
    //   fill it with the appropriate junction character.
    //
    // "Our divider" is determined by fg colour matching `divider_cell`'s fg
    // (index 239) so that pane-painted `│`/`─` characters with their own
    // colour — e.g. the right border of the sidebar's Activity box — are NOT
    // merged into the canvas divider logic. Without this check, a pane's
    // `│` next to a canvas `│` one column right was getting upgraded to
    // `├┤`, producing stray-looking "arrow ticks" on the sidebar edge.
    let is_own = |c: &StyledCell| is_divider_ch(c.ch) && c.fg == Some(239);

    let rows = win.rows as usize;
    let cols = win.cols as usize;
    for y in 0..rows {
        for x in 0..cols {
            let up = y > 0 && is_own(&canvas[y - 1][x]);
            let down = (y + 1) < rows && is_own(&canvas[y + 1][x]);
            let left = x > 0 && is_own(&canvas[y][x - 1]);
            let right = (x + 1) < cols && is_own(&canvas[y][x + 1]);

            let ch = canvas[y][x].ch;
            let new_ch = if is_own(&canvas[y][x]) {
                // Upgrade an existing divider to a junction.
                match (up, down, left, right) {
                    (true, true, true, true) => '┼',
                    (true, true, true, false) => '┤',
                    (true, true, false, true) => '├',
                    (true, false, true, true) => '┴',
                    (false, true, true, true) => '┬',
                    _ => ch,
                }
            } else if is_blank(&canvas[y][x]) && (up || down) && (left || right) {
                // Fill a blank crossing cell where vertical and horizontal
                // dividers meet (e.g. the center of a 2×2 pane grid).
                match (up, down, left, right) {
                    (true, true, true, true) => '┼',
                    (true, true, true, false) => '┤',
                    (true, true, false, true) => '├',
                    (true, false, true, true) => '┴',
                    (false, true, true, true) => '┬',
                    (true, false, true, false) => '┘',
                    (true, false, false, true) => '└',
                    (false, true, true, false) => '┐',
                    (false, true, false, true) => '┌',
                    _ => ch,
                }
            } else {
                ch
            };
            canvas[y][x].ch = new_ch;
        }
    }
}

fn is_blank(cell: &StyledCell) -> bool {
    cell.ch == ' ' && cell.fg.is_none() && cell.bg.is_none()
}

fn is_divider_ch(ch: char) -> bool {
    matches!(ch, '│' | '─' | '┼' | '┤' | '├' | '┴' | '┬')
}

fn divider_cell(ch: char) -> StyledCell {
    StyledCell {
        ch,
        fg: Some(239),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assemble_blank_when_no_panes() {
        let win = WindowGeom { cols: 3, rows: 2 };
        let canvas = assemble(&win, &[]);
        assert_eq!(canvas.len(), 2);
        assert!(canvas.iter().all(|r| r.iter().all(|c| c.ch == ' ')));
    }

    #[test]
    fn assemble_single_pane_no_border() {
        let geom = PaneGeom {
            pane_id: "%1".into(),
            left: 0,
            top: 0,
            width: 3,
            height: 1,
            active: true,
        };
        let pane = PaneContent {
            geom,
            cells: vec![vec![
                StyledCell {
                    ch: 'a',
                    ..Default::default()
                },
                StyledCell {
                    ch: 'b',
                    ..Default::default()
                },
                StyledCell {
                    ch: 'c',
                    ..Default::default()
                },
            ]],
        };
        let canvas = assemble(&WindowGeom { cols: 3, rows: 1 }, &[pane]);
        assert_eq!(canvas[0][0].ch, 'a');
        assert_eq!(canvas[0][1].ch, 'b');
        assert_eq!(canvas[0][2].ch, 'c');
    }
}
