#!/usr/bin/env bash
# Hero: one-shot single-frame capture of the 4-pane ideal-day layout.
#
# Usage:  scenario.sh <output-dir> [extra capture args...]

set -euo pipefail

OUT="${1:?usage: scenario.sh <output-dir> [extra capture args...]}"
shift
EXTRA_ARGS=("$@")

source "$(cd "$(dirname "$0")/../common" && pwd)/_lib.sh"

# Focus the Codex Waiting pane for visual variety (attention
# indicator + "permission_required" wait reason).
export FOCUS=PANE_WAITING

setup "hero"
trap cleanup EXIT

mkdir -p "$OUT"

build_layout
paint_stream "$MAIN_PANE" \
    "$ROOT/fixtures/scenarios/hero/main-pane.stream"

# Seed the Activity tab with recent tool calls + tasks on the
# focused (Codex Waiting) pane — the fix/login-redirect
# investigation right before it hit the permission prompt.
cat >> "$FOCUSED_LOG" <<'ACTIVITY'
14:02|TaskCreate|#1 reproduce the deep-link regression
14:02|TaskCreate|#2 preserve window.location.search through the redirect
14:02|TaskCreate|#3 add a failing test for the deep-link flow
14:02|TaskCreate|#4 update the migration doc for v13 router
14:03|Read|packages/web/src/login.tsx
14:03|Read|packages/web/src/router.ts
14:03|Grep|deep-link
14:03|TaskUpdate|completed #1
14:03|Read|packages/web/tests/login.test.tsx
14:03|TaskUpdate|in_progress #3
14:04|Edit|packages/web/src/login.tsx
14:04|Bash|npm test -- login
14:04|TaskUpdate|completed #3
14:04|Read|packages/web/src/auth/session.ts
14:04|TaskUpdate|in_progress #2
14:05|Edit|packages/web/src/login.tsx
14:05|Bash|npm run build
ACTIVITY

start_sidebar

capture_single
