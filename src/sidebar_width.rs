//! Parsing and resolution of the `@sidebar_width` tmux option.
//!
//! The option accepts absolute columns (`30`), a percentage of the window
//! width (`15%`), or a percentage with a clamp range in columns —
//! `15%[20,40]`, `15%[20]` / `15%[20,]` (min only), `15%[,40]` (max only).
//! The percentage is resolved once when the sidebar pane is created
//! (`cli::toggle`); the clamp bounds are additionally re-enforced by the
//! TUI on terminal-driven resizes (`app::run` via [`ClampEnforcer`]),
//! since tmux rescales panes proportionally on terminal resize without
//! regard for the bounds. Manual pane-border drags are respected.

use crate::tmux;

/// Parsed `@sidebar_width` tmux option value: either absolute columns
/// (passed to tmux verbatim) or a percentage of the window width with an
/// optional clamp range appended in brackets — `15%[20,40]` clamps to
/// 20–40 columns, `15%[20]` / `15%[20,]` set only a minimum, and
/// `15%[,40]` only a maximum.
#[derive(Debug, Eq, PartialEq)]
pub enum SidebarWidth {
    Columns(String),
    Percent {
        pct: u32,
        min: Option<u32>,
        max: Option<u32>,
    },
}

/// Parse the raw `@sidebar_width` option value. A value without `%` is
/// absolute columns. Malformed input degrades gracefully instead of
/// breaking the sidebar — an unparseable percentage falls back to the
/// default 15 and a malformed clamp range is ignored as a whole — but
/// each degradation also returns a warning so the typo is surfaced in
/// the tmux status line rather than silently masked.
pub fn parse_sidebar_width(setting: &str) -> (SidebarWidth, Option<String>) {
    let setting = setting.trim();
    match setting.split_once('%') {
        Some((pct, clamp)) => {
            let (pct, pct_warning) = match pct.trim().parse() {
                Ok(pct) => (pct, None),
                Err(_) => (
                    15,
                    Some(format!("invalid @sidebar_width {setting:?}; using 15%")),
                ),
            };
            let clamp = clamp.trim();
            let ((min, max), clamp_warning) = if clamp.is_empty() {
                ((None, None), None)
            } else if let Some(bounds) = parse_width_clamp(clamp) {
                (bounds, None)
            } else {
                (
                    (None, None),
                    Some(format!(
                        "ignoring malformed clamp in @sidebar_width {setting:?}"
                    )),
                )
            };
            (
                SidebarWidth::Percent { pct, min, max },
                pct_warning.or(clamp_warning),
            )
        }
        None => {
            let warning = (!setting.chars().all(|c| c.is_ascii_digit()))
                .then(|| format!("invalid @sidebar_width {setting:?}"));
            (SidebarWidth::Columns(setting.to_string()), warning)
        }
    }
}

/// Parse the `[min,max]` suffix of a percentage width. Accepts
/// `[20,40]`, `[20]`, `[20,]`, and `[,40]`; an empty bound means "no
/// bound on that side". Returns `None` when the suffix is not a
/// well-formed bracket range or any non-empty bound fails to parse, so
/// a typo never half-applies the clamp.
fn parse_width_clamp(clamp: &str) -> Option<(Option<u32>, Option<u32>)> {
    let inner = clamp.strip_prefix('[')?.strip_suffix(']')?;
    let bound = |s: &str| {
        let s = s.trim();
        if s.is_empty() {
            Some(None)
        } else {
            s.parse().ok().map(Some)
        }
    };
    match inner.split_once(',') {
        Some((min, max)) => Some((bound(min)?, bound(max)?)),
        None => Some((bound(inner)?, None)),
    }
}

/// Turn a percentage width into a column count for `split-window -l`,
/// clamped to the optional `[min,max]` range. If the window width is
/// unknown the bare percentage is returned so tmux resolves it itself
/// (the clamp cannot be applied without the width). When min exceeds
/// max, min wins — a readable sidebar beats a strict maximum — but the
/// result is always capped so the pane being split keeps at least one
/// column plus the border, since tmux answers an unsatisfiable `-l`
/// with a useless one-column sliver instead of an error.
pub fn resolve_percent_width(
    window_width: u32,
    pct: u32,
    min: Option<u32>,
    max: Option<u32>,
) -> String {
    if window_width == 0 {
        return format!("{}%", pct.max(1));
    }
    let mut width =
        (u64::from(window_width) * u64::from(pct) / 100).min(u64::from(u32::MAX)) as u32;
    if let Some(max) = max {
        width = width.min(max);
    }
    if let Some(min) = min {
        width = width.max(min);
    }
    width
        .clamp(1, window_width.saturating_sub(2).max(1))
        .to_string()
}

/// Read the clamp bounds configured on `@sidebar_width`, if any. The
/// option is resolved from `target`'s scope (pane → window → session →
/// global inheritance) via format expansion, matching how the toggle
/// path reads it at pane creation — a `show -gv` lookup would see only
/// the global value and miss window/session-local overrides. Only a
/// percentage width carries bounds; absolute columns and malformed
/// settings yield none (warnings are the toggle path's job).
pub fn configured_clamp_bounds(target: &str) -> (Option<u32>, Option<u32>) {
    let setting = tmux::display_message(target, &format!("#{{{}}}", tmux::SIDEBAR_WIDTH));
    if setting.is_empty() {
        return (None, None);
    }
    match parse_sidebar_width(&setting).0 {
        SidebarWidth::Percent { min, max, .. } => (min, max),
        SidebarWidth::Columns(_) => (None, None),
    }
}

/// Query the width of the tmux window containing `target` (a pane or
/// window id), or 0 when the query fails.
pub fn window_width_of(target: &str) -> u32 {
    tmux::display_message(target, "#{window_width}")
        .parse()
        .unwrap_or(0)
}

/// Decide the corrective width for a resized sidebar pane, if the new
/// width violates the clamp bounds. Min wins when the bounds conflict,
/// mirroring [`resolve_percent_width`].
fn clamp_correction(width: u32, min: Option<u32>, max: Option<u32>) -> Option<u32> {
    match (min, max) {
        (Some(min), _) if width < min => Some(min),
        (_, Some(max)) if width > max => Some(max),
        _ => None,
    }
}

/// Re-enforces the `@sidebar_width` clamp bounds across pane resize
/// events, distinguishing the two things that look identical to the
/// pane's pty: a terminal-window resize (tmux rescales panes
/// proportionally, ignoring the bounds — snap back) and a manual
/// pane-border drag (a deliberate user choice — leave alone, even past
/// the bounds). The tell is the tmux window width: it changes with the
/// terminal but stays fixed during a border drag.
pub struct ClampEnforcer {
    min: Option<u32>,
    max: Option<u32>,
    last_window_width: u32,
}

impl ClampEnforcer {
    /// Build from the configured `@sidebar_width` bounds, priming the
    /// window-width baseline via `target`. Unbounded widths skip the
    /// baseline query and disable enforcement entirely.
    pub fn from_option(target: &str) -> Self {
        let (min, max) = configured_clamp_bounds(target);
        let last_window_width = if min.is_some() || max.is_some() {
            window_width_of(target)
        } else {
            0
        };
        Self {
            min,
            max,
            last_window_width,
        }
    }

    /// Whether any bound is configured; when false, callers can skip
    /// the per-resize window-width query altogether.
    pub fn is_active(&self) -> bool {
        self.min.is_some() || self.max.is_some()
    }

    /// Handle one pane resize event: returns the corrective width only
    /// when the window width moved since the last event (a terminal
    /// resize, not a border drag) and the new pane width violates the
    /// bounds. A failed window-width query (0) never triggers a
    /// correction and keeps the previous baseline.
    pub fn correction(&mut self, pane_width: u32, window_width: u32) -> Option<u32> {
        if window_width == 0 {
            return None;
        }
        let window_changed = window_width != self.last_window_width;
        self.last_window_width = window_width;
        if !window_changed {
            return None;
        }
        clamp_correction(pane_width, self.min, self.max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn percent(pct: u32, min: Option<u32>, max: Option<u32>) -> SidebarWidth {
        SidebarWidth::Percent { pct, min, max }
    }

    /// Parse a setting and assert no warning was produced.
    fn parse_ok(setting: &str) -> SidebarWidth {
        let (width, warning) = parse_sidebar_width(setting);
        assert_eq!(warning, None, "unexpected warning for {setting:?}");
        width
    }

    /// Parse a setting and assert a warning was produced.
    fn parse_warned(setting: &str) -> SidebarWidth {
        let (width, warning) = parse_sidebar_width(setting);
        assert!(warning.is_some(), "expected a warning for {setting:?}");
        width
    }

    #[test]
    fn parse_sidebar_width_absolute_columns_pass_through() {
        assert_eq!(parse_ok("30"), SidebarWidth::Columns("30".to_string()));
        assert_eq!(parse_ok(" 42 "), SidebarWidth::Columns("42".to_string()));
    }

    #[test]
    fn parse_sidebar_width_plain_percentage() {
        assert_eq!(parse_ok("15%"), percent(15, None, None));
        assert_eq!(parse_ok(" 25% "), percent(25, None, None));
    }

    #[test]
    fn parse_sidebar_width_percentage_with_clamp_range() {
        assert_eq!(parse_ok("15%[20,40]"), percent(15, Some(20), Some(40)));
    }

    #[test]
    fn parse_sidebar_width_min_only_forms() {
        assert_eq!(parse_ok("15%[20]"), percent(15, Some(20), None));
        assert_eq!(parse_ok("15%[20,]"), percent(15, Some(20), None));
    }

    #[test]
    fn parse_sidebar_width_max_only_form() {
        assert_eq!(parse_ok("15%[,40]"), percent(15, None, Some(40)));
    }

    #[test]
    fn parse_sidebar_width_empty_bounds_mean_no_clamp() {
        assert_eq!(parse_ok("15%[]"), percent(15, None, None));
        assert_eq!(parse_ok("15%[,]"), percent(15, None, None));
    }

    #[test]
    fn parse_sidebar_width_invalid_percentage_falls_back_with_warning() {
        assert_eq!(parse_warned("abc%"), percent(15, None, None));
        assert_eq!(parse_warned("%"), percent(15, None, None));
    }

    #[test]
    fn parse_sidebar_width_malformed_clamp_is_ignored_with_warning() {
        assert_eq!(parse_warned("15%[20,40"), percent(15, None, None));
        assert_eq!(parse_warned("15%20,40]"), percent(15, None, None));
        assert_eq!(parse_warned("15%[a,b]"), percent(15, None, None));
    }

    #[test]
    fn parse_sidebar_width_partially_malformed_clamp_is_ignored_whole() {
        // One bad bound must not half-apply the clamp: min would survive
        // while the intended max silently vanished.
        assert_eq!(parse_warned("15%[20,4o]"), percent(15, None, None));
        assert_eq!(parse_warned("15%[2o,40]"), percent(15, None, None));
        assert_eq!(parse_warned("15%[20,30,40]"), percent(15, None, None));
    }

    #[test]
    fn parse_sidebar_width_non_numeric_columns_warn_but_pass_through() {
        // A clamp attached to an absolute width (or any other junk) still
        // reaches tmux verbatim, but the user gets a warning instead of a
        // silently dead toggle key.
        assert_eq!(
            parse_warned("30[20,40]"),
            SidebarWidth::Columns("30[20,40]".to_string())
        );
    }

    #[test]
    fn resolve_percent_width_computes_columns_without_clamp() {
        assert_eq!(resolve_percent_width(200, 15, None, None), "30");
    }

    #[test]
    fn resolve_percent_width_clamps_to_min_and_max() {
        // 15% of 100 = 15 → raised to min 20
        assert_eq!(resolve_percent_width(100, 15, Some(20), Some(40)), "20");
        // 15% of 400 = 60 → lowered to max 40
        assert_eq!(resolve_percent_width(400, 15, Some(20), Some(40)), "40");
        // 15% of 200 = 30 → within range, untouched
        assert_eq!(resolve_percent_width(200, 15, Some(20), Some(40)), "30");
    }

    #[test]
    fn resolve_percent_width_one_sided_bounds() {
        assert_eq!(resolve_percent_width(100, 15, Some(20), None), "20");
        assert_eq!(resolve_percent_width(400, 15, Some(20), None), "60");
        assert_eq!(resolve_percent_width(400, 15, None, Some(40)), "40");
        assert_eq!(resolve_percent_width(100, 15, None, Some(40)), "15");
    }

    #[test]
    fn resolve_percent_width_min_wins_over_smaller_max() {
        assert_eq!(resolve_percent_width(200, 15, Some(50), Some(40)), "50");
    }

    #[test]
    fn resolve_percent_width_falls_back_to_bare_percent_without_window_width() {
        assert_eq!(resolve_percent_width(0, 15, Some(20), Some(40)), "15%");
        // Zero never leaves this function, even on the fallback path.
        assert_eq!(resolve_percent_width(0, 0, None, None), "1%");
    }

    #[test]
    fn resolve_percent_width_never_returns_zero_columns() {
        assert_eq!(resolve_percent_width(4, 15, None, None), "1");
        assert_eq!(resolve_percent_width(200, 0, None, None), "1");
    }

    #[test]
    fn resolve_percent_width_caps_at_window_width_minus_border() {
        // A min larger than the window must not produce an unsatisfiable
        // `-l`: tmux would answer with a one-column sliver pane.
        assert_eq!(resolve_percent_width(60, 15, Some(80), None), "58");
        // Same cap for oversized percentages.
        assert_eq!(resolve_percent_width(200, 150, None, None), "198");
        // Tiny windows degrade to one column rather than underflowing.
        assert_eq!(resolve_percent_width(2, 50, Some(80), None), "1");
    }

    #[test]
    fn resolve_percent_width_survives_absurd_percentages() {
        // window_width * pct would overflow u32; the u64 math caps it
        // instead of panicking (debug) or wrapping (release).
        assert_eq!(resolve_percent_width(200, 4_000_000_000, None, None), "198");
    }

    #[test]
    fn clamp_correction_snaps_only_out_of_range_widths() {
        assert_eq!(clamp_correction(15, Some(20), Some(40)), Some(20));
        assert_eq!(clamp_correction(60, Some(20), Some(40)), Some(40));
        assert_eq!(clamp_correction(30, Some(20), Some(40)), None);
        assert_eq!(clamp_correction(30, None, None), None);
    }

    #[test]
    fn clamp_correction_min_wins_over_smaller_max() {
        assert_eq!(clamp_correction(45, Some(50), Some(40)), Some(50));
    }

    fn enforcer(min: Option<u32>, max: Option<u32>, window_width: u32) -> ClampEnforcer {
        ClampEnforcer {
            min,
            max,
            last_window_width: window_width,
        }
    }

    #[test]
    fn enforcer_corrects_on_terminal_resize() {
        let mut e = enforcer(Some(20), Some(40), 200);
        // Terminal shrank: window 200 → 100, sidebar rescaled to 15.
        assert_eq!(e.correction(15, 100), Some(20));
        // The follow-up event from our own resize-pane sees the same
        // window width and must not fight back.
        assert_eq!(e.correction(20, 100), None);
    }

    #[test]
    fn enforcer_allows_manual_border_drag_past_bounds() {
        let mut e = enforcer(Some(20), Some(40), 200);
        // Border drag: pane width moves but the window width does not.
        assert_eq!(e.correction(45, 200), None);
        assert_eq!(e.correction(10, 200), None);
    }

    #[test]
    fn enforcer_updates_baseline_across_resizes() {
        let mut e = enforcer(Some(20), None, 200);
        // In-bounds terminal resize corrects nothing but moves the
        // baseline, so a later drag at the new width is manual.
        assert_eq!(e.correction(30, 150), None);
        assert_eq!(e.correction(10, 150), None);
        // The next terminal resize is measured against 150, not 200.
        assert_eq!(e.correction(8, 120), Some(20));
    }

    #[test]
    fn enforcer_ignores_failed_window_width_query() {
        let mut e = enforcer(Some(20), Some(40), 200);
        assert_eq!(e.correction(15, 0), None);
        // Baseline survives the failed query.
        assert_eq!(e.correction(15, 100), Some(20));
    }

    #[test]
    fn enforcer_without_bounds_is_inactive() {
        assert!(!enforcer(None, None, 0).is_active());
        assert!(enforcer(Some(20), None, 0).is_active());
        assert!(enforcer(None, Some(40), 0).is_active());
    }
}
