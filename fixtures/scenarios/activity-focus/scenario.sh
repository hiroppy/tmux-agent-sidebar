#!/usr/bin/env bash
# Single-frame snapshot cropped to the Activity tab at the bottom.
#
# Usage:  scenario.sh <output-dir> [extra capture args...]

set -euo pipefail

OUT="${1:?usage: scenario.sh <output-dir> [extra capture args...]}"
shift
EXTRA_ARGS=("$@")

source "$(cd "$(dirname "$0")/../common" && pwd)/_lib.sh"

export FOCUS=PANE_WAITING
# Crop to the sidebar's bottom panel: rows 26..46 (below the agent
# list) × cols 0..46 (sidebar's 46-col width).
export CROP_ROWS=26:46
export CROP_COLS=0:46

setup "activity-focus"
trap cleanup EXIT

mkdir -p "$OUT"

build_layout
paint_stream "$MAIN_PANE" \
    "$ROOT/fixtures/scenarios/hero/main-pane.stream"

# Rich activity log for the focused pane — hits every tool-category
# colour in src/activity.rs.
cat >> "$FOCUSED_LOG" <<'ACTIVITY'
14:01|TaskCreate|#1 reproduce the deep-link regression
14:01|TaskCreate|#2 preserve window.location.search through the redirect
14:01|TaskCreate|#3 add a failing test for the deep-link flow
14:01|TaskCreate|#4 update the migration doc for v13 router
14:02|Read|packages/web/src/login.tsx
14:02|Read|packages/web/src/router.ts
14:02|Grep|deep-link
14:02|TaskUpdate|completed #1
14:03|Read|packages/web/tests/login.test.tsx
14:03|TaskUpdate|in_progress #3
14:03|Edit|packages/web/src/login.tsx
14:03|Bash|npm test -- login
14:03|TaskUpdate|completed #3
14:04|Glob|packages/web/src/**/*.ts
14:04|Agent|Explore #d9e0f1
14:04|Skill|superpowers:using-git-worktrees
14:04|WebFetch|https://reactrouter.com/en/docs
14:04|Read|packages/web/src/auth/session.ts
14:04|TaskUpdate|in_progress #2
14:05|Edit|packages/web/src/login.tsx
14:05|LSP|textDocument/rename
14:05|mcp__playwright__screenshot|login-redirect.png
14:05|NotebookEdit|analysis/redirect-timings.ipynb
14:05|AskUserQuestion|merge to main?
14:05|Bash|npm run build
14:05|EnterWorktree|fix/login-redirect
14:05|Write|packages/web/src/login.tsx
ACTIVITY

start_sidebar

capture_single
