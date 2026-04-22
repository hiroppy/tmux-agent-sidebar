use std::path::PathBuf;

use super::config::{DEFAULT_BRANCH_PREFIX, RemoveMode};
use super::env::{RealEnv, SpawnEnv};
use super::markers::{
    SPAWN_MARKERS_TEMPLATE, SPAWNED_BRANCH_OPTION, SPAWNED_FROM_OPTION, SPAWNED_OPTION,
    SPAWNED_WORKTREE_OPTION, SpawnMarkers,
};
use super::slug::{MAX_COLLISION_ATTEMPTS, pick_unique_slug, slugify, worktree_path_for};

#[derive(Debug, Clone)]
pub struct SpawnRequest {
    pub repo_root: PathBuf,
    pub task_name: String,
    pub session: String,
    pub agent: String,
    pub mode: String,
}

/// Create a worktree, open a new tmux window in it, launch the agent,
/// and stash markers at window scope so the matching `x` flow can find
/// it later. Returns the resulting branch name on success. On any
/// failure past `git worktree add` the worktree is rolled back.
pub fn spawn(req: &SpawnRequest) -> Result<String, String> {
    spawn_with(&RealEnv, req)
}

pub(crate) fn spawn_with<E: SpawnEnv>(env: &E, req: &SpawnRequest) -> Result<String, String> {
    let slug = slugify(&req.task_name);
    if slug.is_empty() {
        return Err("name is empty after slugification".into());
    }
    let repo = req
        .repo_root
        .to_str()
        .ok_or("repo root is not valid UTF-8")?;

    let prefix = env
        .branch_prefix()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_BRANCH_PREFIX.to_string());

    let unique = pick_unique_slug(&slug, |s| {
        let branch = format!("{prefix}{s}");
        env.branch_is_free(repo, &branch)
            && worktree_path_for(&req.repo_root, s).is_some_and(|p| env.worktree_path_is_free(&p))
    })
    .ok_or_else(|| format!("no free branch name found after {MAX_COLLISION_ATTEMPTS} attempts"))?;

    let branch = format!("{prefix}{unique}");
    let worktree_path =
        worktree_path_for(&req.repo_root, &unique).ok_or("repo root has no parent directory")?;
    let worktree = worktree_path.to_str().ok_or("worktree path is not UTF-8")?;

    env.worktree_add(repo, worktree, &branch)
        .map_err(|e| format!("git: {e}"))?;

    let (pane_id, window_id) = env
        .new_window(&req.session, worktree, &unique)
        .map_err(|e| {
            let rb = rollback_spawn(env, repo, worktree, &branch, None);
            compose_spawn_error(format!("tmux: {e}"), rb)
        })?;

    // Window scope so sub panes (e.g. Claude Code subagents split from
    // the original) inherit the markers via tmux's option fall-through.
    for (key, value) in [
        (SPAWNED_OPTION, "1"),
        (SPAWNED_FROM_OPTION, repo),
        (SPAWNED_WORKTREE_OPTION, worktree),
        (SPAWNED_BRANCH_OPTION, &branch),
    ] {
        if let Err(e) = env.set_window_option(&window_id, key, value) {
            let rb = rollback_spawn(env, repo, worktree, &branch, Some(&window_id));
            return Err(compose_spawn_error(
                format!("tmux: failed to set {key}: {e}"),
                rb,
            ));
        }
    }

    if let Err(e) = env.send_command(
        &pane_id,
        &super::config::agent_command(&req.agent, &req.mode),
    ) {
        let rb = rollback_spawn(env, repo, worktree, &branch, Some(&window_id));
        return Err(compose_spawn_error(format!("tmux: {e}"), rb));
    }

    Ok(branch)
}

/// Best-effort rollback after a partial spawn. Kills the tmux window
/// (when one was created), removes the git worktree, and deletes the
/// branch ref that `git worktree add -b` created. Each step collects
/// its error so the caller can surface a full picture of what is
/// still lying around on disk / in tmux. Deleting the branch is
/// important: `worktree remove --force` leaves the branch behind,
/// which later spawns would then collide with.
fn rollback_spawn<E: SpawnEnv>(
    env: &E,
    repo: &str,
    worktree_path: &str,
    branch: &str,
    window_id: Option<&str>,
) -> Vec<String> {
    let mut errs = Vec::new();
    if let Some(window_id) = window_id
        && let Err(e) = env.kill_window(window_id)
    {
        errs.push(format!("kill_window: {e}"));
    }
    if let Err(e) = env.worktree_remove(repo, worktree_path) {
        errs.push(format!("worktree_remove: {e}"));
    }
    if let Err(e) = env.branch_delete(repo, branch) {
        errs.push(format!("branch_delete: {e}"));
    }
    errs
}

/// Combine the primary spawn error with any rollback failures so the
/// user sees a single string that covers both the trigger and the
/// state left behind.
fn compose_spawn_error(primary: String, rollback_errs: Vec<String>) -> String {
    if rollback_errs.is_empty() {
        primary
    } else {
        format!(
            "{primary} (rollback incomplete: {})",
            rollback_errs.join("; ")
        )
    }
}

/// Tear down a previously-spawned pane. Runs ALL git cleanup
/// (`worktree remove --force`, then `git branch -D`) BEFORE killing
/// the tmux window so a git failure at any step leaves the window
/// (and its markers) intact — the window is the only UI handle the
/// retry path depends on, so killing it first would strand any
/// leftover git state with no way to finish cleanup from the
/// sidebar. Each git step is skipped when its target is already
/// gone (`worktree_path_exists` / `branch_exists`), which lets
/// retries after a partial-success failure converge.
pub fn remove(pane_id: &str, mode: RemoveMode) -> Result<(), String> {
    remove_with(&RealEnv, pane_id, mode)
}

pub(crate) fn remove_with<E: SpawnEnv>(
    env: &E,
    pane_id: &str,
    mode: RemoveMode,
) -> Result<(), String> {
    let markers = SpawnMarkers::parse(&env.display_message(pane_id, SPAWN_MARKERS_TEMPLATE));
    if !markers.is_spawned() {
        return Err("pane was not created by sidebar spawn".into());
    }
    if markers.worktree_path.is_empty() {
        return Err("spawned worktree path is unset".into());
    }
    if markers.branch.is_empty() {
        return Err("spawned branch is unset".into());
    }
    if markers.window_id.is_empty() {
        return Err("could not resolve window id".into());
    }

    if mode == RemoveMode::WindowAndWorktree {
        if env.worktree_path_exists(&markers.worktree_path) {
            env.worktree_remove(&markers.from_repo, &markers.worktree_path)
                .map_err(|e| format!("git: {e}"))?;
        }
        // `git worktree remove` leaves the branch ref behind; drop
        // it here, before `kill_window`, so a failure still leaves
        // the window as a retry handle.
        if env.branch_exists(&markers.from_repo, &markers.branch) {
            env.branch_delete(&markers.from_repo, &markers.branch)
                .map_err(|e| format!("git: {e}"))?;
        }
    }
    env.kill_window(&markers.window_id)
        .map_err(|e| format!("tmux: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod env_tests {
    use super::*;
    use std::cell::RefCell;
    use std::path::Path;

    #[derive(Default)]
    struct FakeEnv {
        calls: RefCell<Vec<String>>,
        set_option_calls: RefCell<usize>,
        fail_set_option_at: Option<usize>,
        fail_kill_window: bool,
        fail_worktree_remove: bool,
        fail_branch_delete: bool,
        fail_send_command: bool,
        display_output: Option<String>,
        /// Programs `worktree_path_is_free`. `None` = default false
        /// (the worktree exists on disk). `Some(true)` = the path was
        /// already cleaned up (e.g. on a retry after a partial
        /// failure) so the remove flow should skip the git step.
        worktree_path_already_gone: Option<bool>,
        /// Programs `branch_exists` for remove tests. `None` / `false`
        /// = branch still present (default), `Some(true)` = the
        /// branch was already dropped by a previous partial success
        /// so the remove flow should skip `git branch -D`.
        branch_already_gone: Option<bool>,
    }

    impl FakeEnv {
        fn log(&self, s: String) {
            self.calls.borrow_mut().push(s);
        }
        fn calls(&self) -> Vec<String> {
            self.calls.borrow().clone()
        }
    }

    impl SpawnEnv for FakeEnv {
        fn branch_prefix(&self) -> Option<String> {
            None
        }
        fn branch_is_free(&self, _repo: &str, _branch: &str) -> bool {
            true
        }
        fn branch_exists(&self, _repo: &str, _branch: &str) -> bool {
            !self.branch_already_gone.unwrap_or(false)
        }
        fn worktree_path_is_free(&self, _path: &Path) -> bool {
            true
        }
        fn worktree_path_exists(&self, _path: &str) -> bool {
            !self.worktree_path_already_gone.unwrap_or(false)
        }
        fn worktree_add(&self, repo: &str, path: &str, branch: &str) -> Result<(), String> {
            self.log(format!("worktree_add({repo},{path},{branch})"));
            Ok(())
        }
        fn worktree_remove(&self, repo: &str, path: &str) -> Result<(), String> {
            self.log(format!("worktree_remove({repo},{path})"));
            if self.fail_worktree_remove {
                Err("worktree_remove failed".into())
            } else {
                Ok(())
            }
        }
        fn branch_delete(&self, repo: &str, branch: &str) -> Result<(), String> {
            self.log(format!("branch_delete({repo},{branch})"));
            if self.fail_branch_delete {
                Err("branch_delete failed".into())
            } else {
                Ok(())
            }
        }
        fn new_window(
            &self,
            session: &str,
            cwd: &str,
            name: &str,
        ) -> Result<(String, String), String> {
            self.log(format!("new_window({session},{cwd},{name})"));
            Ok(("%1".into(), "@1".into()))
        }
        fn kill_window(&self, window_id: &str) -> Result<(), String> {
            self.log(format!("kill_window({window_id})"));
            if self.fail_kill_window {
                Err("kill_window failed".into())
            } else {
                Ok(())
            }
        }
        fn set_window_option(&self, window: &str, key: &str, _value: &str) -> Result<(), String> {
            let idx = *self.set_option_calls.borrow();
            *self.set_option_calls.borrow_mut() += 1;
            self.log(format!("set_window_option({window},{key})"));
            if Some(idx) == self.fail_set_option_at {
                Err(format!("set {key} failed"))
            } else {
                Ok(())
            }
        }
        fn send_command(&self, target: &str, command: &str) -> Result<(), String> {
            self.log(format!("send_command({target},{command})"));
            if self.fail_send_command {
                Err("send_command failed".into())
            } else {
                Ok(())
            }
        }
        fn display_message(&self, _pane_id: &str, _template: &str) -> String {
            self.display_output
                .clone()
                .unwrap_or_else(|| "1\n/r\n/r-worktrees/task\nagent/task\n@1\n".into())
        }
    }

    fn sample_req() -> SpawnRequest {
        SpawnRequest {
            repo_root: PathBuf::from("/r"),
            task_name: "task".into(),
            session: "sess".into(),
            agent: "claude".into(),
            mode: "default".into(),
        }
    }

    fn has_call(calls: &[String], prefix: &str) -> bool {
        calls.iter().any(|c| c.starts_with(prefix))
    }

    #[test]
    fn spawn_happy_path_sets_all_markers_then_sends_command() {
        let env = FakeEnv::default();
        let branch = spawn_with(&env, &sample_req()).expect("spawn should succeed");
        assert_eq!(branch, "agent/task");
        let calls = env.calls();
        assert!(has_call(&calls, "worktree_add("));
        assert!(has_call(&calls, "new_window("));
        assert_eq!(*env.set_option_calls.borrow(), 4);
        assert!(has_call(&calls, "send_command(%1,claude"));
        assert!(
            !has_call(&calls, "kill_window("),
            "no rollback on happy path"
        );
    }

    #[test]
    fn spawn_rolls_back_when_first_marker_fails() {
        let env = FakeEnv {
            fail_set_option_at: Some(0),
            ..FakeEnv::default()
        };
        let err = spawn_with(&env, &sample_req()).expect_err("spawn must fail");
        assert!(err.contains(SPAWNED_OPTION), "error mentions marker: {err}");
        let calls = env.calls();
        assert!(
            has_call(&calls, "kill_window(@1)"),
            "kill_window rollback: {calls:?}"
        );
        assert!(
            has_call(&calls, "worktree_remove("),
            "worktree_remove rollback: {calls:?}"
        );
        assert!(
            !has_call(&calls, "send_command("),
            "send_command must not run after marker failure: {calls:?}"
        );
    }

    #[test]
    fn spawn_rolls_back_when_middle_marker_fails() {
        let env = FakeEnv {
            fail_set_option_at: Some(1),
            ..FakeEnv::default()
        };
        let err = spawn_with(&env, &sample_req()).expect_err("spawn must fail");
        assert!(
            err.contains(SPAWNED_FROM_OPTION),
            "error mentions second marker: {err}"
        );
        let calls = env.calls();
        assert!(has_call(&calls, "kill_window(@1)"));
        assert!(has_call(&calls, "worktree_remove("));
        assert!(!has_call(&calls, "send_command("));
    }

    #[test]
    fn remove_runs_all_git_steps_before_kill_window() {
        let env = FakeEnv::default();
        remove_with(&env, "%1", RemoveMode::WindowAndWorktree).expect("remove should succeed");
        let calls = env.calls();
        let wt_idx = calls
            .iter()
            .position(|c| c.starts_with("worktree_remove"))
            .expect("worktree_remove called");
        let branch_idx = calls
            .iter()
            .position(|c| c.starts_with("branch_delete"))
            .expect("branch_delete called");
        let kill_idx = calls
            .iter()
            .position(|c| c.starts_with("kill_window"))
            .expect("kill_window called");
        assert!(
            wt_idx < branch_idx && branch_idx < kill_idx,
            "git cleanup must precede kill_window so a git failure \
             leaves the window as a retry handle: {calls:?}"
        );
    }

    #[test]
    fn remove_does_not_kill_window_when_worktree_remove_fails() {
        let env = FakeEnv {
            fail_worktree_remove: true,
            ..FakeEnv::default()
        };
        let err =
            remove_with(&env, "%1", RemoveMode::WindowAndWorktree).expect_err("remove must fail");
        assert!(err.contains("worktree_remove failed"), "error: {err}");
        let calls = env.calls();
        assert!(
            !has_call(&calls, "kill_window("),
            "kill_window must not run when git cleanup fails — the window is the only retry handle: {calls:?}"
        );
    }

    #[test]
    fn remove_skips_git_step_when_worktree_already_gone() {
        let env = FakeEnv {
            worktree_path_already_gone: Some(true),
            fail_worktree_remove: true,
            ..FakeEnv::default()
        };
        remove_with(&env, "%1", RemoveMode::WindowAndWorktree)
            .expect("remove should still succeed via kill when worktree path is gone");
        let calls = env.calls();
        assert!(
            !has_call(&calls, "worktree_remove("),
            "worktree_remove must be skipped when the path is already gone: {calls:?}"
        );
        assert!(has_call(&calls, "kill_window(@1)"));
    }

    #[test]
    fn remove_window_only_skips_worktree_remove() {
        let env = FakeEnv::default();
        remove_with(&env, "%1", RemoveMode::WindowOnly).expect("remove should succeed");
        let calls = env.calls();
        assert!(has_call(&calls, "kill_window(@1)"));
        assert!(
            !has_call(&calls, "worktree_remove("),
            "WindowOnly must not touch worktree: {calls:?}"
        );
    }

    #[test]
    fn spawn_rolls_back_when_send_command_fails() {
        let env = FakeEnv {
            fail_send_command: true,
            ..FakeEnv::default()
        };
        let err = spawn_with(&env, &sample_req()).expect_err("spawn must fail");
        assert!(
            err.contains("send_command failed"),
            "primary error surfaced: {err}"
        );
        let calls = env.calls();
        assert!(
            has_call(&calls, "kill_window(@1)"),
            "kill_window rollback on send failure: {calls:?}"
        );
        assert!(
            has_call(&calls, "worktree_remove("),
            "worktree_remove rollback on send failure: {calls:?}"
        );
        assert!(
            has_call(&calls, "branch_delete(/r,agent/task)"),
            "branch_delete rollback on send failure: {calls:?}"
        );
    }

    #[test]
    fn spawn_rolls_back_branch_when_marker_fails() {
        let env = FakeEnv {
            fail_set_option_at: Some(0),
            ..FakeEnv::default()
        };
        spawn_with(&env, &sample_req()).expect_err("spawn must fail");
        let calls = env.calls();
        assert!(
            has_call(&calls, "branch_delete(/r,agent/task)"),
            "rollback must delete the branch `git worktree add -b` created: {calls:?}"
        );
    }

    #[test]
    fn spawn_rolls_back_branch_when_new_window_fails() {
        #[derive(Default)]
        struct NewWindowFailingEnv(FakeEnv);
        impl SpawnEnv for NewWindowFailingEnv {
            fn branch_prefix(&self) -> Option<String> {
                self.0.branch_prefix()
            }
            fn branch_is_free(&self, r: &str, b: &str) -> bool {
                self.0.branch_is_free(r, b)
            }
            fn branch_exists(&self, r: &str, b: &str) -> bool {
                self.0.branch_exists(r, b)
            }
            fn worktree_path_is_free(&self, p: &Path) -> bool {
                self.0.worktree_path_is_free(p)
            }
            fn worktree_path_exists(&self, p: &str) -> bool {
                self.0.worktree_path_exists(p)
            }
            fn worktree_add(&self, r: &str, p: &str, b: &str) -> Result<(), String> {
                self.0.worktree_add(r, p, b)
            }
            fn worktree_remove(&self, r: &str, p: &str) -> Result<(), String> {
                self.0.worktree_remove(r, p)
            }
            fn branch_delete(&self, r: &str, b: &str) -> Result<(), String> {
                self.0.branch_delete(r, b)
            }
            fn new_window(&self, _s: &str, _c: &str, _n: &str) -> Result<(String, String), String> {
                self.0.log("new_window(fail)".into());
                Err("new_window failed".into())
            }
            fn kill_window(&self, w: &str) -> Result<(), String> {
                self.0.kill_window(w)
            }
            fn set_window_option(&self, w: &str, k: &str, v: &str) -> Result<(), String> {
                self.0.set_window_option(w, k, v)
            }
            fn send_command(&self, t: &str, c: &str) -> Result<(), String> {
                self.0.send_command(t, c)
            }
            fn display_message(&self, p: &str, t: &str) -> String {
                self.0.display_message(p, t)
            }
        }

        let env = NewWindowFailingEnv::default();
        let err = spawn_with(&env, &sample_req()).expect_err("spawn must fail");
        assert!(err.contains("new_window failed"));
        let calls = env.0.calls();
        assert!(
            !has_call(&calls, "kill_window("),
            "no window was ever created: {calls:?}"
        );
        assert!(
            has_call(&calls, "worktree_remove("),
            "worktree must be cleaned up after new_window failure: {calls:?}"
        );
        assert!(
            has_call(&calls, "branch_delete(/r,agent/task)"),
            "branch must be deleted after new_window failure: {calls:?}"
        );
    }

    #[test]
    fn spawn_surfaces_rollback_failure_when_kill_window_also_fails() {
        let env = FakeEnv {
            fail_set_option_at: Some(0),
            fail_kill_window: true,
            ..FakeEnv::default()
        };
        let err = spawn_with(&env, &sample_req()).expect_err("spawn must fail");
        assert!(
            err.contains("rollback incomplete"),
            "rollback failure surfaced: {err}"
        );
        assert!(
            err.contains("kill_window"),
            "rollback error names kill_window: {err}"
        );
    }

    #[test]
    fn spawn_surfaces_rollback_failure_when_worktree_remove_also_fails() {
        let env = FakeEnv {
            fail_send_command: true,
            fail_worktree_remove: true,
            ..FakeEnv::default()
        };
        let err = spawn_with(&env, &sample_req()).expect_err("spawn must fail");
        assert!(
            err.contains("send_command failed"),
            "primary error surfaced: {err}"
        );
        assert!(
            err.contains("rollback incomplete"),
            "rollback failure surfaced: {err}"
        );
        assert!(
            err.contains("worktree_remove"),
            "rollback error names worktree_remove: {err}"
        );
    }

    #[test]
    fn remove_rejects_pane_missing_spawned_marker() {
        let env = FakeEnv {
            display_output: Some("\n/r\n/r-worktrees/task\nagent/task\n@1\n".into()),
            ..FakeEnv::default()
        };
        let err =
            remove_with(&env, "%1", RemoveMode::WindowAndWorktree).expect_err("remove must fail");
        assert!(err.contains("not created by sidebar spawn"));
        assert!(!has_call(&env.calls(), "kill_window("));
    }

    #[test]
    fn remove_deletes_branch_between_worktree_and_kill_window() {
        let env = FakeEnv::default();
        remove_with(&env, "%1", RemoveMode::WindowAndWorktree).expect("remove should succeed");
        let calls = env.calls();
        let wt_idx = calls
            .iter()
            .position(|c| c.starts_with("worktree_remove"))
            .expect("worktree_remove called");
        let branch_idx = calls
            .iter()
            .position(|c| c.starts_with("branch_delete"))
            .expect("branch_delete called");
        let kill_idx = calls
            .iter()
            .position(|c| c.starts_with("kill_window"))
            .expect("kill_window called");
        assert!(
            wt_idx < branch_idx && branch_idx < kill_idx,
            "expected worktree → branch → kill order: {calls:?}"
        );
        assert!(has_call(&calls, "branch_delete(/r,agent/task)"));
    }

    #[test]
    fn remove_window_only_does_not_delete_branch() {
        let env = FakeEnv::default();
        remove_with(&env, "%1", RemoveMode::WindowOnly).expect("remove should succeed");
        let calls = env.calls();
        assert!(
            !has_call(&calls, "branch_delete("),
            "WindowOnly must not touch branch: {calls:?}"
        );
    }

    #[test]
    fn remove_rejects_pane_with_empty_branch_marker() {
        let env = FakeEnv {
            display_output: Some("1\n/r\n/r-worktrees/task\n\n@1\n".into()),
            ..FakeEnv::default()
        };
        let err =
            remove_with(&env, "%1", RemoveMode::WindowAndWorktree).expect_err("remove must fail");
        assert!(err.contains("branch is unset"), "error: {err}");
        assert!(!has_call(&env.calls(), "worktree_remove("));
        assert!(!has_call(&env.calls(), "kill_window("));
    }

    #[test]
    fn remove_does_not_kill_window_when_branch_delete_fails() {
        let env = FakeEnv {
            fail_branch_delete: true,
            ..FakeEnv::default()
        };
        let err =
            remove_with(&env, "%1", RemoveMode::WindowAndWorktree).expect_err("remove must fail");
        assert!(err.contains("branch_delete failed"), "error: {err}");
        let calls = env.calls();
        assert!(
            has_call(&calls, "worktree_remove("),
            "worktree_remove ran first: {calls:?}"
        );
        assert!(
            !has_call(&calls, "kill_window("),
            "kill_window must not run when branch_delete fails — \
             the window is the only retry handle for the orphaned \
             branch: {calls:?}"
        );
    }

    #[test]
    fn remove_skips_branch_delete_when_branch_already_gone() {
        // Simulates a retry after a previous partial success already
        // dropped the branch (e.g. branch_delete succeeded but
        // kill_window then failed on a prior attempt). The flow must
        // converge instead of re-running `git branch -D` and erroring
        // on a missing ref.
        let env = FakeEnv {
            worktree_path_already_gone: Some(true),
            branch_already_gone: Some(true),
            fail_branch_delete: true,
            ..FakeEnv::default()
        };
        remove_with(&env, "%1", RemoveMode::WindowAndWorktree)
            .expect("retry should converge once git state is gone");
        let calls = env.calls();
        assert!(
            !has_call(&calls, "branch_delete("),
            "branch_delete must be skipped when branch is gone: {calls:?}"
        );
        assert!(
            !has_call(&calls, "worktree_remove("),
            "worktree_remove must also stay skipped: {calls:?}"
        );
        assert!(has_call(&calls, "kill_window(@1)"));
    }
}
