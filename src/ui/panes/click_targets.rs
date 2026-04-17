use ratatui::layout::Rect;

use super::row_collector::CollectedRows;
use super::{REMOVE_MARKER_HIT_WIDTH, SPAWN_BUTTON};
use crate::state::{AppState, RepoSpawnTarget, SpawnRemoveTarget};

pub(super) fn materialize(
    state: &mut AppState,
    collected: &CollectedRows,
    scroll_offset: usize,
    list_area: Rect,
) {
    let btn_width = SPAWN_BUTTON.len() as u16;
    state.layout.repo_spawn_targets = collected
        .pending_spawn
        .iter()
        .filter_map(|(line_idx, repo_name, repo_root)| {
            if *line_idx < scroll_offset {
                return None;
            }
            let screen_row = (*line_idx - scroll_offset) as u16;
            if screen_row >= list_area.height {
                return None;
            }
            let btn_x = list_area.x + list_area.width.saturating_sub(btn_width);
            let btn_y = list_area.y + screen_row;
            Some(RepoSpawnTarget {
                rect: Rect::new(btn_x, btn_y, btn_width, 1),
                repo_name: repo_name.clone(),
                repo_root: repo_root.clone(),
            })
        })
        .collect();

    state.layout.spawn_remove_targets = collected
        .pending_remove
        .iter()
        .filter_map(|(line_idx, marker_col, pane_id)| {
            if *line_idx < scroll_offset {
                return None;
            }
            let screen_row = (*line_idx - scroll_offset) as u16;
            if screen_row >= list_area.height {
                return None;
            }
            // The `×` sits at the rightmost row column, so the hit
            // region can only extend leftward. Extending by
            // `REMOVE_MARKER_HIT_WIDTH - 1` keeps the glyph at the
            // right edge of the click rect with two columns of slack
            // to its left (which normally covers the space or port
            // digits just left of the marker).
            let btn_x = list_area
                .x
                .saturating_add(marker_col.saturating_sub(REMOVE_MARKER_HIT_WIDTH - 1));
            let btn_y = list_area.y + screen_row;
            Some(SpawnRemoveTarget {
                rect: Rect::new(btn_x, btn_y, REMOVE_MARKER_HIT_WIDTH, 1),
                pane_id: pane_id.clone(),
            })
        })
        .collect();
}
