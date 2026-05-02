#!/usr/bin/env bash
# Single-frame snapshot of the pet at its home position. Forces the
# count of running agents to zero so the cat stays at PET_HOME_X = 1.
#
# The cat may briefly take a few right-walking steps before the next
# refresh_interval (1 s, src/app.rs) picks up the @pane_status overrides
# and running_count drops to 0 — at which point the cat WalkLefts back
# home. ~3 s of extra sleep past start_sidebar's built-in 2 s gives the
# round trip plenty of headroom before capture.
#
# Usage: scenario.sh <output-dir> [extra capture args...]

set -euo pipefail

OUT="${1:?usage: scenario.sh <output-dir> [extra capture args...]}"
shift
EXTRA_ARGS=("$@")

source "$(cd "$(dirname "$0")/../common" && pwd)/_lib.sh"

# Crop to the pet scene band: 5 rows immediately above the bottom
# panel. Layout (src/ui/mod.rs:60-78) splits the canvas into pane list
# (Min 1) | pet band (Length 5) | bottom panel (Length 20). At
# canvas height 46 (build_layout), PET_SCENE_HEIGHT = 5, and
# default BOTTOM_PANEL_HEIGHT = 20, the band is rows 21..25 inclusive.
# CROP_ROWS is end-exclusive.
export CROP_ROWS=21:26
export CROP_COLS=0:46

setup "pet-idle"
trap cleanup EXIT

mkdir -p "$OUT"

build_layout

# Force running_count to 0 by overriding the two `running` panes.
# Only @pane_status matters for the pet — @pane_attention /
# @pane_wait_reason set by _seed_pane don't influence tick_pet.
tmux set-option -t "$MAIN_PANE"      -p @pane_status waiting
tmux set-option -t "$PANE_RUNNING_2" -p @pane_status waiting

enable_pet
start_sidebar

# 3 s headroom. The override is set BEFORE start_sidebar, so init_state's
# first state.refresh() (src/app/setup.rs) sees @pane_status=waiting on
# both running panes — the cat normally stays Idle from tick 1. If the
# tmux options haven't propagated by the time init_state reads, the cat
# may take a few WalkRight steps to pet_x ≈ 20 before the next 1 s
# refresh_interval (src/app.rs) picks up the override; the worst-case
# WalkLeft back to PET_HOME_X = 1 takes ~13 ticks (~2.6 s) at
# 200 ms/tick. 3 s covers both paths.
sleep 3

capture_single
