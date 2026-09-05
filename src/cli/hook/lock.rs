//! Per-pane serialization for hook handlers.
//!
//! Every hook fire is its own short-lived process, and each handler is a
//! read-modify-write sequence over tmux pane options with no atomicity:
//! `Stop` can read an empty child list while `SubagentStart` is appending
//! to it, or a final `SubagentStop` can cache "turn settled" while
//! `UserPromptSubmit` opens the next turn. Ordering-tolerant handlers
//! cover events that merely arrive late; they cannot cover two handlers
//! interleaving their reads and writes, and tmux offers no
//! compare-and-swap to close that from inside a handler. Holding an
//! advisory lock per pane for the duration of `handle_event` makes each
//! hook observe the state its predecessor left behind.
//!
//! The wait is bounded: a handler wedged in a hung notification daemon
//! must not turn every later hook for that pane into a lost event, so
//! after `LOCK_WAIT` the hook proceeds unlocked. Degraded, but live.
//!
//! The lock file is never removed. `flock` binds to the open file
//! description, so two processes locking different inodes at the same
//! path would exclude nothing; every teardown that deletes per-pane
//! files targets the activity log by exact path, and this file lives in
//! a directory of its own, private to the user, for that reason.

use std::env;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use crate::desktop_notification::{
    DESKTOP_NOTIFICATION_PROBE_TIMEOUT, DESKTOP_NOTIFICATION_TIMEOUT,
};

/// The longest a handler can hold the lock. Its tmux calls are quick;
/// the one desktop notification it may send is the bound — the backend
/// probe and the send are each killed on their own timeout.
const fn longest_locked_hold() -> Duration {
    DESKTOP_NOTIFICATION_PROBE_TIMEOUT.saturating_add(DESKTOP_NOTIFICATION_TIMEOUT)
}

/// Upper bound on how long a hook waits for its predecessor on the same
/// pane. It must outlast `longest_locked_hold`, or a contender would give
/// up and run unlocked exactly while a stalled notifier keeps the lock —
/// reopening the interleavings the lock exists to prevent. Twice that,
/// so a hook queued behind two consecutive stalled-notifier hooks (Stop,
/// then the final SubagentStop) still serializes; deeper queues under a
/// stalled notifier degrade to running unlocked. No hook is registered
/// with an explicit agent-side timeout, so Claude Code's 60s default
/// bounds this from above.
const LOCK_WAIT: Duration = longest_locked_hold().saturating_mul(2);
/// Interval between non-blocking acquisition attempts.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Directory holding the lock files, private to the invoking user the
/// way tmux keeps its own socket directory. tmux numbers panes per
/// server, so `%1` repeats across users on a shared host: a world-shared
/// name would let the first user's 0644 file turn every later user's
/// open into `EACCES`, and those hooks would run unserialized for good
/// since the file is never removed. Two servers of the same user still
/// share a name for the same pane id; that only over-serializes.
///
/// `None` when the directory cannot be made ours — unwritable base, a
/// symlink planted in its place, or owned by someone else — and the
/// caller then proceeds unlocked like the rest of this module. The
/// directory-swap race that remains is the one tmux accepts too.
pub(super) fn lock_dir_under(base: &Path) -> Option<PathBuf> {
    // `getuid` cannot fail.
    let uid = unsafe { libc::getuid() };
    let dir = base.join(format!("tmux-agent-sidebar-{uid}"));
    fs::create_dir_all(&dir).ok()?;
    let meta = fs::symlink_metadata(&dir).ok()?;
    if !meta.is_dir() || meta.uid() != uid {
        return None;
    }
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).ok()?;
    Some(dir)
}

/// Lock file name for `pane_id`, encoded like `activity::log_file_path`.
pub(super) fn lock_file_name(pane_id: &str) -> String {
    format!("hook{}.lock", pane_id.replace('%', "_"))
}

/// Guard for the per-pane advisory lock. Dropping it closes the file
/// description, which releases the lock in the kernel — including when
/// the hook process exits abnormally. The file is held for that `Drop`
/// alone; nothing reads it.
pub(super) struct PaneLock {
    _file: Option<File>,
}

impl PaneLock {
    /// Whether the lock was actually obtained, as opposed to the bounded
    /// wait expiring or the lock file being unopenable.
    #[cfg(test)]
    pub(super) fn held(&self) -> bool {
        self._file.is_some()
    }
}

/// Serialize with other hooks on `pane`, waiting at most `LOCK_WAIT`.
pub(super) fn acquire(pane: &str) -> PaneLock {
    let Some(dir) = lock_dir_under(&env::temp_dir()) else {
        return PaneLock { _file: None };
    };
    try_lock_until(&dir.join(lock_file_name(pane)), Instant::now() + LOCK_WAIT)
}

/// Poll for an exclusive lock on `path` until `deadline`. Never blocks
/// indefinitely and never fails the hook: an unopenable path or an
/// expired deadline yield an unheld guard and the caller proceeds.
pub(super) fn try_lock_until(path: &Path, deadline: Instant) -> PaneLock {
    let Ok(file) = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
    else {
        return PaneLock { _file: None };
    };
    loop {
        match try_flock(&file) {
            Ok(()) => return PaneLock { _file: Some(file) },
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                if Instant::now() >= deadline {
                    return PaneLock { _file: None };
                }
                thread::sleep(POLL_INTERVAL);
            }
            Err(_) => return PaneLock { _file: None },
        }
    }
}

fn try_flock(file: &File) -> io::Result<()> {
    // `file` outlives the call, so the descriptor is valid for its duration.
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_lock(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("tmux-agent-hook-lock-test-{name}.lock"))
    }

    fn scratch_base(name: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("tmux-agent-lock-dir-test-{name}"));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn lock_file_name_encodes_pane_id_like_the_activity_log() {
        assert_eq!(lock_file_name("%5"), "hook_5.lock");
    }

    #[test]
    fn lock_wait_outlasts_the_longest_locked_hold_and_fits_the_agent_timeout() {
        let hold = DESKTOP_NOTIFICATION_PROBE_TIMEOUT + DESKTOP_NOTIFICATION_TIMEOUT;
        assert!(
            LOCK_WAIT > hold,
            "a contender must wait longer than a stalled notifier can hold the lock ({hold:?})"
        );
        assert!(
            LOCK_WAIT + hold < Duration::from_secs(60),
            "waiting must never run into the agent's default hook timeout"
        );
    }

    #[test]
    fn lock_dir_is_private_to_the_user() {
        let base = scratch_base("private");
        let dir = lock_dir_under(&base).expect("a writable base must yield a lock dir");
        let uid = unsafe { libc::getuid() };
        assert!(
            dir.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with(&format!("-{uid}")),
            "the directory name must carry the uid so users never share one: {dir:?}"
        );
        let meta = fs::metadata(&dir).unwrap();
        assert_eq!(
            meta.mode() & 0o777,
            0o700,
            "the lock dir must be readable by its owner only"
        );
        assert_eq!(meta.uid(), uid);
        // A second call finds the directory already in place and keeps it.
        assert_eq!(lock_dir_under(&base).as_deref(), Some(dir.as_path()));
    }

    #[test]
    fn lock_dir_rejects_a_symlink_planted_in_its_place() {
        // Another user could pre-create our name as a symlink into a
        // directory they control; `symlink_metadata` sees the link itself
        // rather than following it. (Ownership by a different uid is not
        // reproducible single-user, so only the symlink arm is pinned.)
        let base = scratch_base("symlink");
        let elsewhere = base.join("elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        let uid = unsafe { libc::getuid() };
        std::os::unix::fs::symlink(&elsewhere, base.join(format!("tmux-agent-sidebar-{uid}")))
            .unwrap();
        assert!(
            lock_dir_under(&base).is_none(),
            "a planted symlink must not be accepted as the lock dir"
        );
    }

    #[test]
    fn lock_is_released_on_drop_and_can_be_reacquired() {
        let path = scratch_lock("reacquire");
        let first = try_lock_until(&path, Instant::now() + Duration::from_secs(1));
        assert!(first.held());
        drop(first);
        let second = try_lock_until(&path, Instant::now() + Duration::from_secs(1));
        assert!(second.held(), "dropping the guard must release the lock");
    }

    #[test]
    fn contender_waits_out_the_deadline_then_proceeds_unlocked() {
        // `flock` binds to the open file description, so a second open of
        // the same path within one process contends for real.
        let path = scratch_lock("contend");
        let holder = try_lock_until(&path, Instant::now() + Duration::from_secs(1));
        assert!(holder.held());

        let wait = Duration::from_millis(60);
        let started = Instant::now();
        let contender = try_lock_until(&path, started + wait);
        let elapsed = started.elapsed();

        assert!(!contender.held(), "a held lock must not be granted twice");
        assert!(
            elapsed >= wait,
            "the contender must wait out its deadline before giving up, waited {elapsed:?}"
        );
        assert!(
            elapsed < wait * 10,
            "the bounded wait must not stretch far past the deadline, waited {elapsed:?}"
        );
    }

    #[test]
    fn unopenable_lock_path_yields_an_unheld_guard_without_panicking() {
        let path = scratch_lock("missing-dir").join("nested").join("pane.lock");
        let guard = try_lock_until(&path, Instant::now() + Duration::from_secs(1));
        assert!(!guard.held());
    }
}
