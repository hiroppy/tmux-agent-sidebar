use ratatui::layout::Rect;

use super::{REMOVE_MARKER_HIT_WIDTH, SPAWN_BUTTON};
use crate::state::{AppState, RepoSpawnTarget, SpawnRemoveTarget};

pub(super) fn materialize(
    state: &mut AppState,
    pending_spawn: Vec<(usize, String, String)>,
    pending_remove: Vec<(usize, u16, String)>,
    scroll_offset: usize,
    list_area: Rect,
) {
    let btn_width = SPAWN_BUTTON.len() as u16;
    state.layout.repo_spawn_targets = pending_spawn
        .into_iter()
        .filter_map(|(line_idx, repo_name, repo_root)| {
            if line_idx < scroll_offset {
                return None;
            }
            let screen_row = (line_idx - scroll_offset) as u16;
            if screen_row >= list_area.height {
                return None;
            }
            let btn_x = list_area.x + list_area.width.saturating_sub(btn_width);
            let btn_y = list_area.y + screen_row;
            Some(RepoSpawnTarget {
                rect: Rect::new(btn_x, btn_y, btn_width, 1),
                repo_name,
                repo_root,
            })
        })
        .collect();

    state.layout.spawn_remove_targets = pending_remove
        .into_iter()
        .filter_map(|(line_idx, marker_col, pane_id)| {
            if line_idx < scroll_offset {
                return None;
            }
            let screen_row = (line_idx - scroll_offset) as u16;
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
                pane_id,
            })
        })
        .collect();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    fn test_list_area() -> Rect {
        Rect {
            x: 10,
            y: 5,
            width: 40,
            height: 10,
        }
    }

    #[test]
    fn materialize_places_spawn_targets_at_absolute_coords() {
        let mut state = AppState::new("%0".into());
        let list_area = test_list_area();

        materialize(
            &mut state,
            vec![(0, "repo".into(), "/tmp/repo".into())],
            Vec::new(),
            0,
            list_area,
        );

        assert_eq!(state.layout.repo_spawn_targets.len(), 1);
        let target = &state.layout.repo_spawn_targets[0];
        assert_eq!(target.repo_name, "repo");
        assert_eq!(target.repo_root, "/tmp/repo");
        // Spawn button sits at the right edge of list_area (x+width-btn_width),
        // on the list_area.y row when line_idx == scroll_offset.
        let btn_width = SPAWN_BUTTON.len() as u16;
        assert_eq!(target.rect.x, list_area.x + list_area.width - btn_width);
        assert_eq!(target.rect.y, list_area.y);
        assert_eq!(target.rect.width, btn_width);
        assert_eq!(target.rect.height, 1);
    }

    #[test]
    fn materialize_places_remove_targets_with_marker_hit_width() {
        let mut state = AppState::new("%0".into());
        let marker_col: u16 = 30;
        let list_area = test_list_area();

        materialize(
            &mut state,
            Vec::new(),
            vec![(2, marker_col, "%42".into())],
            0,
            list_area,
        );

        assert_eq!(state.layout.spawn_remove_targets.len(), 1);
        let target = &state.layout.spawn_remove_targets[0];
        assert_eq!(target.pane_id, "%42");
        // screen_row = 2, so y is list_area.y + 2.
        assert_eq!(target.rect.y, list_area.y + 2);
        assert_eq!(target.rect.width, REMOVE_MARKER_HIT_WIDTH);
        assert_eq!(target.rect.height, 1);
        // The glyph column shifts leftward by REMOVE_MARKER_HIT_WIDTH - 1.
        let expected_x = list_area.x + marker_col.saturating_sub(REMOVE_MARKER_HIT_WIDTH - 1);
        assert_eq!(target.rect.x, expected_x);
    }

    #[test]
    fn materialize_filters_out_lines_above_scroll_offset() {
        let mut state = AppState::new("%0".into());
        let list_area = test_list_area();

        materialize(
            &mut state,
            vec![
                (0, "above".into(), "/repo/above".into()),
                (5, "visible".into(), "/repo/visible".into()),
            ],
            vec![(0, 20, "%above".into()), (5, 20, "%visible".into())],
            3,
            list_area,
        );

        assert_eq!(state.layout.repo_spawn_targets.len(), 1);
        assert_eq!(state.layout.repo_spawn_targets[0].repo_name, "visible");
        assert_eq!(state.layout.spawn_remove_targets.len(), 1);
        assert_eq!(state.layout.spawn_remove_targets[0].pane_id, "%visible");
    }

    #[test]
    fn materialize_filters_out_lines_beyond_visible_area() {
        let mut state = AppState::new("%0".into());
        let list_area = test_list_area();
        // height is 10 → rows [0, 10). line_idx 15 (with scroll_offset=0)
        // maps to screen_row=15 which is >= height=10, so it must drop.
        materialize(
            &mut state,
            vec![
                (0, "inside".into(), "/repo/inside".into()),
                (15, "outside".into(), "/repo/outside".into()),
            ],
            vec![(0, 20, "%inside".into()), (15, 20, "%outside".into())],
            0,
            list_area,
        );

        assert_eq!(state.layout.repo_spawn_targets.len(), 1);
        assert_eq!(state.layout.repo_spawn_targets[0].repo_name, "inside");
        assert_eq!(state.layout.spawn_remove_targets.len(), 1);
        assert_eq!(state.layout.spawn_remove_targets[0].pane_id, "%inside");
    }

    #[test]
    fn materialize_with_empty_collected_clears_targets() {
        let mut state = AppState::new("%0".into());
        // Pre-seed layout targets so we can prove materialize overwrites them.
        state.layout.repo_spawn_targets = vec![RepoSpawnTarget {
            rect: Rect::new(0, 0, 1, 1),
            repo_name: "stale".into(),
            repo_root: "/stale".into(),
        }];
        state.layout.spawn_remove_targets = vec![SpawnRemoveTarget {
            rect: Rect::new(0, 0, 1, 1),
            pane_id: "%stale".into(),
        }];

        materialize(&mut state, Vec::new(), Vec::new(), 0, test_list_area());

        assert!(state.layout.repo_spawn_targets.is_empty());
        assert!(state.layout.spawn_remove_targets.is_empty());
    }
}
