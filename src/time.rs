//! System-clock helpers shared across hook dispatch, TUI refresh, and the
//! desktop notification dedup store. Each caller previously hand-rolled
//! the same `SystemTime::now().duration_since(UNIX_EPOCH)` incantation,
//! which meant every new call site had to reinvent the clock-skew
//! fallback. Centralising here keeps the fallback uniform (`0` on
//! skew) and makes the "we depend on the wall clock" surface grep-able.

use std::time::{SystemTime, UNIX_EPOCH};

/// Wall-clock seconds since the Unix epoch, with `0` as the fallback
/// when the clock is earlier than the epoch (practically impossible,
/// but the `Result` shape forces us to handle it).
pub fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Wall-clock milliseconds since the Unix epoch, same fallback as
/// [`now_epoch_secs`]. Used for the per-run notification id so that two
/// rapid SessionStart events on the same pane get distinct ids.
pub fn now_epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secs_and_millis_reflect_the_same_clock() {
        let secs = now_epoch_secs();
        let ms = now_epoch_millis();
        // Both read the wall clock in quick succession; ms/1000 must sit
        // within a couple of seconds of the secs reading, otherwise one
        // of them is reading a different source.
        assert!(
            ms / 1000 >= secs && ms / 1000 <= secs + 2,
            "secs={secs}, ms={ms}"
        );
    }
}
