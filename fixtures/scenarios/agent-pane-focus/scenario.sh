#!/usr/bin/env bash
# Single-frame snapshot cropped to the sidebar's agent list — shows
# one focused pane entry (Codex, waiting on a permission prompt) in
# detail. Companion to activity-focus / git-focus but for the top
# panel.
#
# Usage:  scenario.sh <output-dir> [extra capture args...]

set -euo pipefail

OUT="${1:?usage: scenario.sh <output-dir> [extra capture args...]}"
shift
EXTRA_ARGS=("$@")

source "$(cd "$(dirname "$0")/../common" && pwd)/_lib.sh"

export FOCUS=PANE_WAITING
# Hide the bottom tab so the crop region is all agent-list rows.
export BOTTOM_HEIGHT=0
# Crop to the sidebar: cols 0..46 (sidebar width), rows 0..26 cover
# the filter bar + repo header + all four agent rows.
export CROP_ROWS=0:26
export CROP_COLS=0:46

setup "agent-pane-focus"
trap cleanup EXIT

mkdir -p "$OUT"

build_layout
paint_stream "$MAIN_PANE" \
    "$ROOT/fixtures/scenarios/hero/main-pane.stream"

start_sidebar

capture_single
