#!/usr/bin/env bash
# Build website assets from scratch: scenario → frames → PNGs → WebM.
#
# Not part of default CI. Requires:
#   - cargo (release binary is rebuilt to pick up any fixes)
#   - node + npm (Playwright + sharp installed under website/)
#   - a working local tmux
#
# Usage: scripts/build-assets.sh

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/website/src/assets/captures"
TMP="$(mktemp -d -t tas-build.XXXX)"
trap 'rm -rf "$TMP"' EXIT

cd "$ROOT"

# Preflight
command -v tmux    >/dev/null || { echo "build-assets: tmux not on PATH"; exit 1; }
command -v node    >/dev/null || { echo "build-assets: node not on PATH"; exit 1; }

echo "==> cargo build --release"
cargo build --release --quiet

mkdir -p "$OUT"

# Render a static single-frame scenario. Produces <name>.png in $OUT.
# Expects the scenario.sh to write <framesdir>/<name>.html.
render_static() {
    local name="$1"
    local framesdir="$TMP/$name"
    mkdir -p "$framesdir"
    echo "==> [$name] scenario"
    "$ROOT/fixtures/scenarios/$name/scenario.sh" "$framesdir"

    echo "==> [$name] html → png"
    ( cd "$ROOT/website" && node "$ROOT/scripts/render-frames.mjs" "$framesdir" )

    cp "$framesdir/$name.png" "$OUT/$name.png"
}

render_hero() {
    render_static hero
    echo "==> [hero] og:image"
    ( cd "$ROOT/website" && node "$ROOT/scripts/hero-compose.mjs" \
        "$OUT/hero.png" "$OUT/og-image.png" )
    # Also publish the og:image under website/public/ so crawlers can
    # fetch it at /<base>/og-image.png (the absolute URL referenced by
    # the meta tags in astro.config.mjs).
    cp "$OUT/og-image.png" "$ROOT/website/public/og-image.png"
}

render_hero
render_static agent-pane-focus
render_static activity-focus
render_static git-focus
render_static worktree-spawn

echo
echo "==> done: $OUT"
ls -lh "$OUT"
