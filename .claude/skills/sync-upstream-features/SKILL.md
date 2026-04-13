---
name: sync-upstream-features
description: Investigate whether the latest hook events and features from Claude Code / Codex CLI are covered by tmux-agent-sidebar's current implementation, and report gaps. Use on requests like "check diff with upstream", "look for new hooks", "check changelog for unsupported features". Does not implement anything.
---

# Sync Upstream Features — Gap Reporter

A skill that fetches the latest documentation and changelogs from Claude Code and Codex CLI, and reports gaps against this project's implementation. Does not implement anything — only outputs a gap list.

## How This Project Works

Before reporting, you need to understand this project's architecture.

### Hook Event Reception

This project uses Claude Code / Codex **settings.json hooks** to receive events. `hook.sh` receives JSON from stdin and passes it to the Rust binary's `hook` subcommand.

Mapping between upstream hook events and internal event names used by this project:

| Upstream Hook Event | Internal Event Name | Notes |
|-------------------|-------------------|-------|
| `SessionStart` | `session-start` | |
| `SessionEnd` | `session-end` | |
| `UserPromptSubmit` | `user-prompt-submit` | |
| `Notification` | `notification` | |
| `Stop` | `stop` | |
| `StopFailure` | `stop-failure` | |
| `SubagentStart` | `subagent-start` | |
| `SubagentStop` | `subagent-stop` | |
| `PostToolUse` | `activity-log` | **Important**: `PreToolUse`/`PostToolUse` are not used directly. Instead, a custom `activity-log` event passes `tool_name`, `tool_input`, `tool_response` from the `PostToolUse` hook |

### Items to Exclude from Report

The following are unrelated to the sidebar monitoring TUI and should NOT be reported as gaps:

- **`PreToolUse` / `PostToolUse` / `PostToolUseFailure`**: Already handled via `activity-log`. No need to handle these directly
- **`PermissionRequest`**: For permission UI control. Sidebar only displays, doesn't need this
- **`PreCompact` / `PostCompact`**: Compaction doesn't affect sidebar
- **`InstructionsLoaded`**: CLAUDE.md loading is unrelated to sidebar
- **`ConfigChange`**: Config change monitoring is outside sidebar scope
- **`FileChanged`**: File change monitoring is outside sidebar scope
- **`Elicitation` / `ElicitationResult`**: MCP elicitation is outside sidebar scope
- **`session_id`**, **`transcript_path`** fields: Information not used by sidebar

**Note**: Do NOT exclude `WorktreeCreate` / `WorktreeRemove`. The sidebar displays worktree information, so these events are useful for monitoring.

### Items to Report

Only report gaps relevant to sidebar agent monitoring:

- **New status-changing events**: Events that affect agent state (running/waiting/idle/error)
- **Permission modes**: Missing variants in `PermissionMode` enum
- **Notification types**: New notification types that affect status display
- **Tool tracking**: Missing entries in `activity.rs` color mapping or `label.rs` label extraction
- **Codex adapter**: Newly supported events in Codex CLI (only if Codex actually sends the event)
- **JSON fields**: Overlooked fields in already-handled events that directly improve sidebar display
- **New hook events**: Events related to agent lifecycle or state tracking (e.g., `TaskCreated`, `TaskCompleted`, `PermissionDenied`, `TeammateIdle`)

### Known Deferred Gaps

These gaps are intentionally left unfixed. Report them only when the trigger condition is met — otherwise include them in the coverage table as `Partial (deferred)` with a one-line pointer to this section instead of repeating the full analysis.

- **`TaskCreated` extra fields (`teammate_name`, `team_name`, `task_description`)**
  - **Current state**: `src/adapter/claude.rs` captures only `task_id` and `task_subject`. The `src/cli/hook.rs` handler for `AgentEvent::TaskCreated` is a no-op (`"Redundant with activity-log; TaskCreate tool use already recorded"`) because the `TaskCreate` tool's `PostToolUse` already writes the activity log entry via the `activity-log` path.
  - **Why deferred**: With the handler doing nothing, capturing the extra fields would be dead code. The fields have no consumer today.
  - **Trigger to revisit**: Claude Code **Agents Teams** feature becomes a target for the sidebar — e.g. showing per-teammate task assignment in the activity log or a new UI panel for team state. When that work starts, capture `teammate_name` / `team_name` / `task_description` and thread them into whichever layer needs them (likely `src/cli/label.rs` for a richer `TaskCreate` label, or a new field on `AgentEvent::TaskCreated` consumed by a new handler).

## Procedure

### 1. Fetch Upstream Documentation

Fetch latest information from the following URLs:

| Source | URL | Focus |
|--------|-----|-------|
| Claude Code Changelog | `https://code.claude.com/docs/en/changelog` | New hook events, permission modes, tools |
| Claude Code Hooks Reference | `https://code.claude.com/docs/en/hooks` | JSON schema for each hook event |
| Codex CLI Features | `https://developers.openai.com/codex/cli/features` | Execution modes, flags |
| Codex Releases | `https://github.com/openai/codex/releases` | New features, new events |

### 2. Read Current Implementation

Identify supported features from the following files:

| Check Item | File | What to Look For |
|-----------|------|-----------------|
| Supported hook events | `src/adapter/claude.rs` | `parse()` match arms |
| Codex supported events | `src/adapter/codex.rs` | `parse()` match arms |
| Internal event definitions | `src/event.rs` | `AgentEvent` enum variants |
| Permission modes | `src/tmux.rs` | `PermissionMode` enum and `from_str()` |
| Tracked tools | `src/activity.rs` | `tool_color_index()` match arms |
| Labeled tools | `src/cli/label.rs` | `extract_tool_label()` match arms |
| Event handlers | `src/cli/hook.rs` | `handle_event()` match arms |

### 3. Report Gaps

Report only gaps matching "Items to Report." Do NOT include items from "Items to Exclude from Report."

#### Categories

- **Hook Events**: Events present upstream but missing from adapter that would be useful for sidebar monitoring
- **Permission Modes**: Missing variants in `PermissionMode` enum
- **Notification Types**: New notification types
- **JSON Fields**: Overlooked fields in already-handled events that could improve display
- **Tools**: Tools not covered in `activity.rs` / `label.rs`
- **Codex-specific**: Codex adapter gaps (only if the event is confirmed to be actually sent)

### 4. Assign Priority

Assign priority to each gap:

- **High**: Status display is wrong or information is missing (e.g., missing permission mode, overlooked error type)
- **Medium**: Display is correct but additional information could improve it (e.g., new hook event support, new tool color)
- **Low**: Nice to have but low impact (e.g., additional label extraction, features not yet widely adopted)

### 5. Output Format

Output the report in the following structure.

#### 5.1 Coverage Table

First, output a table of all upstream hook events with current coverage status. Include ALL upstream events, including those in "Items to Exclude."

Format:

| Hook Event | Added in Version | Status | Notes |
|-----------|-----------------|--------|-------|
| `SessionStart` | (initial) | Covered | |
| `TaskCreated` | v2.1.84 | Not covered | ... |
| `PreToolUse` | (initial) | Out of scope | Handled via activity-log |

- **Added in Version**: The version when the event was added to Claude Code. Identify from changelog. Use `(initial)` for events present from the start. Use `Unknown` if unidentifiable.
- **Status**: `Covered` / `Not covered` / `Partial` / `Out of scope`
  - `Covered`: Match arm exists in adapter's `parse()`
  - `Not covered`: No match arm in adapter, and the event is useful for sidebar monitoring
  - `Partial`: Match arm exists but has field omissions or field name mismatches
  - `Out of scope`: Not relevant to sidebar monitoring (matches "Items to Exclude")

Also output the following tables:

**Permission Modes:**

| Mode | Status | Notes |
|------|--------|-------|
| `default` | Covered | |
| `dontAsk` | Not covered | Falls back to Default |

**Tools (Color Mapping + Label Extraction):**

Show only not-covered / partial tools.

| Tool | Added in Version | Color | Label | Notes |
|------|-----------------|-------|-------|-------|
| `PowerShell` | v2.1.84 | Not covered | Not covered | Windows only |
| `TaskList` | (initial) | Covered | Not covered | |

#### 5.2 Gap Details

For items marked `Not covered` / `Partial` in the coverage table (excluding `Out of scope`), output details by priority.

Each gap should include:
- **Item name**
- **Added in version**
- **Upstream spec**: Brief description
- **Current state**
- **Affected files**

#### 5.3 Summary

Rules for coverage table accuracy:

1. **List adapter match arms before writing**: Before writing the table, enumerate ALL string literals in `src/adapter/claude.rs`'s `parse()` match arms (`"session-start"`, `"session-end"`, etc.). Do not mark an event as "Covered" unless it appears in this enumeration.
2. **Cross-table consistency check**: Verify no contradictions between the coverage table and gap details.
3. **All-category completeness check**: Verify no gaps are missing for permission modes and tools.
