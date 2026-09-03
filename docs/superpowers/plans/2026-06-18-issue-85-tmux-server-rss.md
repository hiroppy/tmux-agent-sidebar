# Issue 85 Tmux Server RSS Mitigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce the tmux server memory growth triggered by the sidebar refresh path on tmux 3.2a by removing `#{q:...}` from the hot `list-panes` query and sanitizing the few tmux pane values that now round-trip through the raw-delimited path, while keeping pane parsing correct and preserving existing UI behavior.

**Architecture:** The fix stays local to the tmux query layer and the hook writes that feed it. Replace the current quoted `list-panes -a -F` output with a delimiter that does not rely on tmux's `q` modifier, then parse the raw fields in Rust and keep the existing session/pane filtering logic unchanged. Add a small amount of cleanup in the tmux value sanitizer only where the hook-writes can still collide with the new delimiter, including the `StopFailure` wait-reason path, and back the change with parser tests plus a tmux 3.2a regression check if the local harness is still usable.

**Tech Stack:** Rust 2024, tmux, cargo test, cargo fmt, cargo build --release, GitHub CLI for issue/PR follow-up.

**Spec:** [`docs/superpowers/specs/2026-06-18-issue-85-tmux-server-rss-design.md`](../specs/2026-06-18-issue-85-tmux-server-rss-design.md)

---

## File Structure

### Modify

- `src/tmux/query.rs` - replace the hot `#{q:...}` `list-panes` format with a raw delimiter format, add raw-field parsing helpers, and update unit tests.
- `src/cli/mod.rs` - extend `sanitize_tmux_value` if the new delimiter can appear in hook-written tmux values.
- `src/cli/hook/context/location.rs`, `src/cli/hook/handlers/attention.rs`, `src/cli/hook/handlers/subagent.rs`, and `src/cli/hook/handlers/run.rs` - sanitize pane cwd, worktree name/branch, session id, wait reasons, subagent lists, and stop-failure reasons before they are written back into tmux options that the raw query path reads.
- `docs/superpowers/specs/2026-06-18-issue-85-tmux-server-rss-design.md` - keep the design writeup aligned with the final implementation shape.

### Test

- `src/tmux/query.rs` unit tests for the raw parser and the existing pane-field semantics.
- `src/cli/mod.rs` sanitizer tests if the sanitizer changes.
- `cargo test`
- `cargo clippy`
- `cargo fmt --check`
- `cargo build --release`

---

## Work Split For Parallel Agents

Use fresh agents for the following disjoint slices:

1. **Query-path agent** - owns `src/tmux/query.rs` only. This agent removes the `q` modifier, introduces the raw delimiter parser, updates the field fixtures, and adds regression tests.
2. **Sanitizer agent** - owns `src/cli/mod.rs`, `src/cli/hook/context/location.rs`, `src/cli/hook/handlers/attention.rs`, `src/cli/hook/handlers/subagent.rs`, and `src/cli/hook/handlers/run.rs`. This agent checks whether the new delimiter needs to be stripped from tmux-stored values and updates the sanitizer/tests accordingly.
3. **Docs/verification agent** - owns the spec markdown file and this plan. This agent keeps the issue writeup and the implementation plan consistent with the code change, then prepares the issue/PR follow-up text once the code is green.

Do not let these agents touch each other's files. If an agent discovers that its slice depends on a different write area, stop and hand the dependency back to the controller before editing.

---

## Task 1: Define the raw tmux query format

**Files:**
- Modify: `src/tmux/query.rs`

The hot refresh path currently uses `pane_format()` to build a `list-panes -a -F` string where every field is wrapped in `#{q:...}`. That is the path implicated by issue #85, so this task removes tmux's quoting from the query itself and switches the parser to a delimiter that survives raw values.

- [ ] **Step 1: Replace the q-wrapped format builder**

Change `pane_format()` from:

```rust
fn pane_format() -> String {
    [
        q("session_name"),
        q("window_id"),
        q("window_index"),
        q("window_name"),
        q("window_active"),
        q("automatic-rename"),
        q("pane_active"),
        q(PANE_STATUS),
        q(PANE_ATTENTION),
        q(PANE_AGENT),
        q(PANE_NAME),
        q("pane_current_path"),
        q("pane_current_command"),
        q(PANE_ROLE),
        q("pane_id"),
        q(PANE_PROMPT),
        q(PANE_PROMPT_SOURCE),
        q(PANE_STARTED_AT),
        q(PANE_WAIT_REASON),
        q("pane_pid"),
        q(PANE_SUBAGENTS),
        q(PANE_CWD),
        q(PANE_PERMISSION_MODE),
        q(PANE_WORKTREE_NAME),
        q(PANE_WORKTREE_BRANCH),
        q(PANE_SESSION_ID),
        q(SPAWNED_OPTION),
        q(PANE_BG_CMD),
    ]
    .join("|")
}

fn q(field: &str) -> String {
    format!("#{{q:{field}}}")
}
```

to a raw format that uses a delimiter not present in the normal hook-written values, such as ASCII unit separator:

```rust
const TMUX_FIELD_DELIMITER: char = '\x1f';

fn pane_format() -> String {
    [
        "session_name",
        "window_id",
        "window_index",
        "window_name",
        "window_active",
        "automatic-rename",
        "pane_active",
        PANE_STATUS,
        PANE_ATTENTION,
        PANE_AGENT,
        PANE_NAME,
        "pane_current_path",
        "pane_current_command",
        PANE_ROLE,
        "pane_id",
        PANE_PROMPT,
        PANE_PROMPT_SOURCE,
        PANE_STARTED_AT,
        PANE_WAIT_REASON,
        "pane_pid",
        PANE_SUBAGENTS,
        PANE_CWD,
        PANE_PERMISSION_MODE,
        PANE_WORKTREE_NAME,
        PANE_WORKTREE_BRANCH,
        PANE_SESSION_ID,
        SPAWNED_OPTION,
        PANE_BG_CMD,
    ]
    .join(&TMUX_FIELD_DELIMITER.to_string())
}
```

Keep the list of fields unchanged. Only the delimiter/escaping strategy changes.

- [ ] **Step 2: Add a raw-field splitter**

Keep the old backslash-aware splitter only if some tests still need it. For the production path, update the splitter to consume the new delimiter without treating backslashes specially:

```rust
fn split_tmux_fields(line: &str) -> Vec<String> {
    line.split(TMUX_FIELD_DELIMITER)
        .map(ToString::to_string)
        .collect()
}
```

Update the hot parse sites in `build_session_hierarchy`, `parse_pane_line`, and `pane_output_needs_process_snapshot` to call the raw splitter for the production format.

- [ ] **Step 3: Update the field-index comments if needed**

If the parser no longer uses `|`-joined fixture strings in production, keep the existing absolute indices but rewrite the comments so they describe the raw-delimiter contract instead of `#{q:...}` escaping.

- [ ] **Step 4: Run the focused parser test**

Run:

```bash
cargo test tmux::query -- --nocapture
```

Expected: parser tests still pass after the format swap.

- [ ] **Step 5: Commit**

```bash
git add src/tmux/query.rs
git commit -m "fix: remove q escaping from hot tmux query path"
```

---

## Task 2: Align tmux value sanitization with the new delimiter

**Files:**
- Modify: `src/cli/mod.rs`

The hook path already normalizes `|` and `\n` before writing tmux pane values. If the new raw delimiter is not one of those characters, this task is a no-op except for confirming the existing sanitizer still matches every field the hot query reads.

- [ ] **Step 1: Re-evaluate whether the sanitizer needs to change**

If the new delimiter is unit separator, the hook-written pane metadata that round-trips through the raw query path still needs to strip it before storage. Keep `sanitize_tmux_value` for the existing pipe-delimited activity-log and prompt/background fields, and add a narrower helper for `@pane_cwd`, `@pane_worktree_name`, `@pane_worktree_branch`, `@pane_session_id`, `@pane_subagents`, and `@pane_wait_reason`:

```rust
pub(crate) fn sanitize_tmux_query_value(s: &str) -> String {
    s.replace(['\n', '\x1f'], " ")
}

pub(crate) fn sanitize_tmux_value(s: &str) -> String {
    s.replace(['\n', '|', '\x1f'], " ")
}
```

- [ ] **Step 2: Keep the sanitizer tests in sync**

If the sanitizer changes, update the tests in `src/cli/mod.rs` to cover the extra replacement:

```rust
#[test]
fn sanitize_query_value_replaces_newlines_and_unit_separators() {
    assert_eq!(sanitize_tmux_query_value("a\x1fb\nc"), "a b c");
}
```

- [ ] **Step 3: Sanitize wait reasons**

Update both wait-reason write paths: `src/cli/hook/handlers/run.rs` must sanitize the `on_stop_failure` `@pane_wait_reason` write, and `src/cli/hook/handlers/attention.rs` must sanitize the `Notification` wait-reason write. Add regressions proving a raw delimiter in each reason is normalized before storage.

- [ ] **Step 4: Run the sanitizer tests**

Run:

```bash
cargo test cli::mod -- --nocapture
```

Expected: sanitizer tests pass, and any added delimiter test plus the `StopFailure` regression prove the new contract.

- [ ] **Step 5: Commit**

```bash
git add src/cli/mod.rs
git commit -m "fix: keep tmux pane values safe for raw query parsing"
```

---

## Task 3: Tighten parser regression coverage

**Files:**
- Modify: `src/tmux/query.rs`

The parser tests currently prove `|` escaping and raw field semantics separately. After the query format changes, the important regression is that the hot path no longer depends on tmux backslash quoting while still preserving long prompts, cwd, subagents, and bg command handling.

- [ ] **Step 1: Rewrite the fixtures for the raw parser contract**

Update the helper fixtures so they reflect the new raw field separator. Keep one test that proves literal `|` survives in fixture values when the parser is fed a hand-built line, add one that proves rows with extra raw-delimited fields are rejected, and add one that proves the production splitter is not looking for backslashes at all.

Example raw fixture:

```rust
fn make_pane_line(fields: &[&str]) -> String {
    fields.join("\x1f")
}
```

- [ ] **Step 2: Add a regression for the hot refresh path**

Add a focused test that assembles a full `list-panes` line with long prompt and background command values, then asserts the same `PaneInfo` fields still parse correctly without `#{q:...}`.

```rust
#[test]
fn parse_pane_line_raw_delimiter_preserves_full_pane_state() {
    let mut fields = full_fields();
    fields[9] = "prompt with | pipes and spaces";
    fields[21] = "tail -f log.txt | grep ERROR";
    let line = make_pane_line(&fields);
    let pane = parse_pane_line(&line).unwrap();
    assert_eq!(pane.prompt, "prompt with | pipes and spaces");
    assert_eq!(pane.bg_shell_cmd.as_deref(), Some("tail -f log.txt | grep ERROR"));
}
```

- [ ] **Step 3: Run the focused query tests again**

Run:

```bash
cargo test tmux::query -- --nocapture
```

Expected: the new raw-delimiter regression passes.

- [ ] **Step 4: Commit**

```bash
git add src/tmux/query.rs
git commit -m "test: cover raw tmux query parsing"
```

---

## Task 4: Verify the issue reproducer and the full build

**Files:**
- None expected unless the regression uncovers a comment or doc mismatch.

Use the preserved tmux 3.2a build under `/tmp/tas85-tmux32-build` if it is still functional. The success condition is that the old reproduction stops showing monotonic RSS growth while the sidebar runs, or at minimum that the new code does not break `cargo test` / `cargo build --release`.

- [ ] **Step 1: Run the Rust test suite**

Run:

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 2: Run lint and format checks**

Run:

```bash
cargo fmt
cargo clippy
cargo fmt --check
```

Expected: formatting is clean and clippy has no new warnings.

- [ ] **Step 3: Build a release binary**

Run:

```bash
cargo build --release
```

Expected: release build succeeds.

- [ ] **Step 4: Re-run the tmux 3.2a harness**

If the `/tmp/tas85-tmux32-build` binaries still exist and the helper harness is still available, rerun the same long-refresh A/B test from the previous sessions against:

1. sidebar attached
2. no-sidebar control

Expected: the attached-server RSS curve is flat or materially flatter than before, and the control remains flat.

- [ ] **Step 5: Commit**

```bash
git add .
git commit -m "test: verify tmux 3.2a rss mitigation"
```

---

## Task 5: PR and issue follow-up

**Files:**
- None

Once the code and verification are done, keep the final review tight and only mention the behavior change that matters for issue #85.

- [ ] **Step 1: Push the branch**

Run:

```bash
git push
```

- [ ] **Step 2: Open or update the PR**

Create the PR against `hiroppy/tmux-agent-sidebar` with a title that names the tmux server RSS fix and a body that explains:

1. the hot refresh path no longer uses `#{q:...}`
2. the parser still preserves pane semantics
3. the issue was reproduced on tmux 3.2a but not on newer tmux

- [ ] **Step 3: Reply on issue #85**

Post a short issue comment that says the fix removes tmux q-escaping from the hot query path, includes the PR link, and notes the tmux 3.2a mitigation status from verification.

- [ ] **Step 4: Close out local work**

If no other follow-up is needed, stop any temporary tmux reproducer or auxiliary agent work and leave the tree clean.
