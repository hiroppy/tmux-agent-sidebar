# Issue 85 Tmux Server RSS Mitigation Design

## Goal

Reduce the tmux server memory growth reported in issue #85 by removing the
`#{q:...}` modifier from the sidebar's hot `list-panes -a -F` refresh path.
Keep the existing pane semantics intact, including prompt parsing, bg-shell
tracking, session grouping, and Codex permission-mode detection.

## Non-Goals

- No change to the sidebar's refresh cadence.
- No rewrite of the activity log format.
- No attempt to fix tmux itself.
- No UI changes beyond whatever test updates fall out of the parser change.

## Diagnosis Summary

The current 1-second refresh loop reaches `src/tmux/query.rs` and builds a
single `tmux list-panes -a -F` format string where every field is wrapped in
`#{q:...}`. Local investigation and the attached issue discussion both point
to tmux 3.2a growing server RSS under this exact pattern, while the no-sidebar
control stays flat.

The load-bearing observation is that the hot path does not need tmux-side
escaping if the parser can consume a delimiter that does not appear in the
stored values. The existing hook-written fields already normalize `|` and
newlines, so the remaining work is to move the query path off `#{q:...}` and
make the parser read raw fields.

## Chosen Approach

Use a raw delimiter in the tmux format output, then split on that delimiter in
Rust. The recommended delimiter is ASCII unit separator (`\x1f`), because it
is invisible in normal output, does not collide with the current `|`-based
tests, and avoids tmux's q-escaping path entirely.

The implementation keeps the set of queried fields unchanged. Only the
transport changes:

1. `pane_format()` emits raw format variables joined by `\x1f`.
2. The parser reads raw split fields instead of `#{q:...}`-escaped fields.
3. Tests are updated so they prove the raw parser still preserves the existing
   pane model.

## Files To Change

- `src/tmux/query.rs`
- `src/cli/mod.rs`
- `src/cli/hook/context/location.rs`
- `src/cli/hook/handlers/attention.rs`
- `src/cli/hook/handlers/subagent.rs`
- `src/cli/hook/handlers/run.rs`

## Detailed Behavior

### tmux query path

`query_sessions_with_process_snapshot()` still performs one `list-panes -a`
call and one optional `ps` snapshot. The only difference is the output format.
Instead of expanding each field through `#{q:...}` and joining with `|`, the
code should:

- define a raw delimiter constant,
- join the same field list with that delimiter,
- split each returned line by that delimiter,
- keep the existing absolute field indexes.

The parser should remain strict about arity. If a line does not contain the
exact expected number of fields, it should still be skipped.

### Sanitization

The hook layer already normalizes tmux-stored values with
`sanitize_tmux_value()`. If the raw delimiter can appear in any tmux-written
value that is read back on the hot path, extend that sanitizer to replace the
delimiter with a space as well. Keep the change minimal and targeted to the
fields written into tmux options.

For pane metadata that round-trips through the raw query path
(`@pane_cwd`, `@pane_worktree_name`, `@pane_worktree_branch`,
`@pane_session_id`, `@pane_subagents`, `@pane_wait_reason`), strip newlines
and the raw delimiter before storing it so the splitter never sees a collision.
That includes `StopFailure`, which writes the wait reason directly from the
error string and therefore needs the same raw-delimiter protection.

### Tests

Update parser fixtures so they exercise the raw delimiter path directly. Add a
regression that proves:

- prompt text still parses,
- `@pane_cwd` still wins over `pane_current_path`,
- bg-shell command text still survives,
- Codex/Claude parsing behavior does not change.

If the sanitizer changes, add a unit test for the delimiter replacement.
For the hook writeback paths, add one focused regression per value family:

- cwd / worktree / session metadata sanitize before storage,
- wait-reason writes sanitize both `Notification` and `StopFailure`,
- subagent list writes sanitize before they are written back.

## Alternatives Considered

### 1. Keep `#{q:...}` and lower the refresh frequency

Rejected. This only reduces pressure; it does not remove the server-side code
path that issue #85 points at.

### 2. Split the query into smaller tmux calls

Rejected. That adds more subprocess traffic and does not address the tmux
format modifier behavior that appears to trigger the growth.

### 3. Change the parser to a raw delimiter and keep the same refresh cadence

Chosen. This is the smallest change that removes the suspect tmux-side
escaping from the hot path while preserving the current model.

## Risks

- Raw delimiter parsing still assumes the delimiter does not appear in normal
  tmux output fields.
- Very unusual paths or command text containing `\x1f` could still collide.
- The tmux 3.2a memory issue may have more than one trigger; the fix should be
  verified against the existing A/B harness if it is still available.

## Success Criteria

- `cargo test` passes.
- `cargo clippy` passes without new warnings.
- `cargo build --release` succeeds.
- The tmux 3.2a reproduction no longer shows the same monotonic server RSS
  growth, or the growth is materially reduced enough to remove the observed
  pane-creation slowdown.
- The issue can be closed with a PR that explains the mitigation clearly.
