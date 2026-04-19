#!/usr/bin/env bash
# Single-frame snapshot showing the worktree-spawn popup open.
#
# Usage:  scenario.sh <output-dir> [extra capture args...]

set -euo pipefail

OUT="${1:?usage: scenario.sh <output-dir> [extra capture args...]}"
shift
EXTRA_ARGS=("$@")

source "$(cd "$(dirname "$0")/../common" && pwd)/_lib.sh"

export FOCUS=MAIN_PANE
# Crop to the popup region (rows 3..15, cols 0..32) — the popup is
# centred over the agent-pane list column.
export CROP_ROWS=3:15
export CROP_COLS=0:32

setup "worktree-spawn"
trap cleanup EXIT

mkdir -p "$OUT"

build_layout
paint_stream "$MAIN_PANE" \
    "$ROOT/fixtures/scenarios/hero/main-pane.stream"

start_sidebar

# Open the spawn popup. Key binding: `n` with Panes focus opens the
# SpawnInput popup against the currently-highlighted pane's repo.
# (See src/app/input.rs around the KeyCode::Char('n') arm.)
tmux send-keys -t "$SIDEBAR_PANE" "n"
sleep 0.4

capture_single
