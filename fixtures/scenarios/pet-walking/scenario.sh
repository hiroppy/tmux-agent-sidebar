#!/usr/bin/env bash
# Single-frame snapshot of the pet mid-walk. Uses build_layout's
# default 2 running agents so the cat enters WalkRight on the very
# first pet tick.
#
# Timing:
#   - start_sidebar sleeps 2 s after launching the binary; ~10 pet
#     ticks fire during that window. Tick 1 transitions Idle → WalkRight
#     and advances pet_x by 1; ticks 2..10 add 2 cols/tick (remaining
#     > 8) → pet_x ≈ 20 when start_sidebar returns.
#   - +1 s extra sleep ≈ 5 more ticks at 2 cols/tick → pet_x ≈ 30,
#     well clear of both endpoints (home = 1, desk park = 37).
#
# Usage: scenario.sh <output-dir> [extra capture args...]

set -euo pipefail

OUT="${1:?usage: scenario.sh <output-dir> [extra capture args...]}"
shift
EXTRA_ARGS=("$@")

source "$(cd "$(dirname "$0")/../common" && pwd)/_lib.sh"

export CROP_ROWS=21:26
export CROP_COLS=0:46

setup "pet-walking"
trap cleanup EXIT

mkdir -p "$OUT"

build_layout

enable_pet
start_sidebar

sleep 1

capture_single
