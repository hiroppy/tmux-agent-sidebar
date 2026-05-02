#!/usr/bin/env bash
# Single-frame snapshot of the pet working at the desk. Uses
# build_layout's default 2 running agents — paper stack height (per
# paper_sprite in src/ui/pet.rs:401) is 2 rows at running_count = 2.
#
# Timing:
#   - Walk completes ~2.4 s after start_sidebar returns (10 ticks
#     in-flight during start_sidebar + ~12 more to reach stop_x = 37).
#   - +5 s extra sleep gives ~2.6 s past walk completion for the
#     paper-shuffle animation to cycle through a stable frame.
#
# Usage: scenario.sh <output-dir> [extra capture args...]

set -euo pipefail

OUT="${1:?usage: scenario.sh <output-dir> [extra capture args...]}"
shift
EXTRA_ARGS=("$@")

source "$(cd "$(dirname "$0")/../common" && pwd)/_lib.sh"

export CROP_ROWS=21:26
export CROP_COLS=0:46

setup "pet-working"
trap cleanup EXIT

mkdir -p "$OUT"

build_layout

enable_pet
start_sidebar

sleep 5

capture_single
