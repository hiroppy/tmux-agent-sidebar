---
name: docs-audit
description: Audit docs/ design documents, specs, and implementation plans against current source code for drift. Outputs a drift report and updates documents. Triggers on "check if docs are up to date", "diff docs vs code", "update design docs", "docs consistency check", "verify specs match source", "check plan completion status". Most effective after code or documentation changes.
---

# Docs Audit — Document vs. Source Code Consistency Check

A skill that audits design documents, specs, and plans under `docs/` against the current source code, reports drift, and updates documents.

## Why This Skill Exists

Code evolves daily, but documentation reflects the design at the time it was written. Enum variants get added, struct fields change, plan checkboxes go un-updated. This drift confuses anyone entering the codebase later. This skill systematically detects such drift.

## Procedure

### Step 1: Discover and Classify Documents

Collect files with `Glob docs/**/*.md` and classify them by the following rules:

| Category | Criteria | Example |
|----------|----------|---------|
| **reference** | `.md` files directly under `docs/` not matching other categories | `event-mapping.md`, `state-management.md` |
| **spec** | Path contains `/specs/` or filename contains `design` | `event-adapter-design.md` |
| **plan** | Path contains `/plans/` | `2026-04-06-event-adapter.md` |
| **todo** | Filename starts with `TODO-` | `TODO-background-shell-detection.md` |

If the user specifies particular files, audit only those files.

### Step 2: Extract Public API from Source Code

Extract type definitions and function signatures from source files. No need to read entire files — use Grep to get only public APIs and conserve context.

**For Rust projects:**
```
Grep: "pub (enum|struct|trait|fn|type)" in src/**/*.rs
```

Extraction targets:
- Enum variant lists and their fields
- Struct field lists
- `pub fn` signatures (parameters and return types)
- Trait definitions

For large files (500+ lines), Read only the types/functions mentioned in the document.

**For other languages (reference):**
- TypeScript: `export (interface|type|class|function|const)`
- Python: `class |def ` at indentation level 0

### Step 3: Audit by Document Type

#### Reference Documents

Check most strictly. Cross-reference ALL of the following against source code:

- **Enum variants**: Do documented variants exist in source? Are source-added variants reflected in docs?
- **Struct fields**: Do field names and types match?
- **Function signatures**: Do parameters and return types match?
- **File paths**: Do referenced files actually exist?
- **Data flow descriptions**: Do statements like "A calls B" or "X writes to Y" match actual code?
- **Tables/mappings**: Are event name mappings and field mappings accurate?

#### Spec Documents

Design specs record "design intent at the time of writing," so they don't need to be as strictly aligned as reference docs. However, major drift is still worth recording.

Check targets:
- How enums/structs defined in spec changed in implementation (variants added/removed/renamed)
- Spec module structure vs. actual module structure
- Spec data flow vs. actual data flow

Drift doesn't necessarily mean the spec is "wrong." The information "it was designed this way, but implemented differently" is valuable.

#### Plan Documents

Focus on task completion status:

- For `- [ ]` (incomplete) tasks: check if the described changes exist in source
  - If they exist → task is implemented but checkbox is not updated
- For `- [x]` (complete) tasks: verify the implementation actually exists
- Determine overall plan status: `fully-implemented` / `partially-implemented` / `not-started`
- Structural diff between code snippets in plan and actual code (compare at type/variant name level, not literal comparison)

#### Todo Documents

Check whether described features have been implemented:
- Do types/functions/files mentioned in the TODO exist in source?
- If implemented: `implemented`; if partial: `partially-implemented`; if not started: `not-started`

### Step 4: Output Drift Report

Output the report in the following structure. Match the language of the document (English docs → English report, Japanese docs → Japanese report).

#### Summary

```
# Document Audit Report

## Summary
- Audited: N documents
- Drift found: X documents (High: A, Medium: B, Low: C)
- No issues: Y documents
```

#### Per-Document Results

For each document:

**For reference / spec:**

```
### docs/event-mapping.md (reference) — N drift items

| Section | Documented | Actual Source | Severity |
|---------|-----------|--------------|----------|
| AgentEvent enum | 9 variants | 11 variants (PermissionDenied, CwdChanged added) | High |
| SessionStart fields | cwd, permission_mode | cwd, permission_mode, worktree, agent_id | High |
```

**For plan:**

```
### docs/plans/2026-04-06-event-adapter.md (plan) — Status: fully-implemented

All 6 tasks implemented. Checkboxes not updated (all still `- [ ]`).
```

**For todo:**

```
### docs/TODO-background-shell-detection.md (todo) — Status: not-started

PaneStatus::Background does not exist in src/tmux.rs.
```

#### Severity Criteria

- **High**: Document contradicts source (nonexistent variant, wrong field, nonexistent file path)
- **Medium**: Document is incomplete (source-added elements not reflected in docs, spec vs. implementation design differences)
- **Low**: Minor drift (plan checkbox not updated, todo completion not reflected)

### Step 5: Propose Fixes

After outputting the report, propose fixes. **Do not fix without user confirmation.**

Fix strategy:

| Document Type | Fix Method |
|--------------|-----------|
| reference | Update content to match current source |
| spec | Either update the body, or add a `## Differences from Implementation` section at the end. Let the user choose |
| plan | Update checkboxes, add `**Status: Completed**` (or `Partially Implemented`) at the top |
| todo | If fully implemented, propose file deletion |

## Notes

- CLAUDE.md is out of scope
- README.md and CHANGELOG.md are also out of scope
- Information "discovered" in source that isn't in the document is not drift. Only report when documented statements contradict source
- Do not perform literal comparison of code snippets. Focus on structural differences (type names, variant names, field names)
