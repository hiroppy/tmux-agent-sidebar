use std::time::Instant;

/// Periodic-refresh bookkeeping. Bundles the wall/monotonic clocks that
/// gate refresh cadence in `state/refresh.rs` (port scan, filter-bar
/// debounce, and the "first port scan completed" flag) so they live as
/// a unit instead of cluttering [`AppState`].
///
/// `session_names` is intentionally NOT here: the polling lives in a
/// dedicated background thread (`session_poll_loop` in `main.rs`) so the
/// TUI thread never performs blocking filesystem I/O.
#[derive(Debug, Clone)]
pub struct RefreshTimers {
    /// Last time a mouse click was processed on the filter bar (debounce).
    pub last_filter_click: Instant,
    /// Timestamp of the last port/command scan.
    pub last_port_refresh: Instant,
    /// Whether the first port scan has completed; the first scan must
    /// always run regardless of the elapsed-time gate.
    pub port_scan_initialized: bool,
}

impl Default for RefreshTimers {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            last_filter_click: now,
            last_port_refresh: now,
            port_scan_initialized: false,
        }
    }
}
