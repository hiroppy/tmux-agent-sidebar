---
name: regenerate-captures
description: Regenerate the Starlight website's capture PNGs (hero / agent-pane-focus / activity-focus / git-focus / worktree-spawn / og-image) from the tmux scenario fixtures. Trigger on "regenerate captures", "rebuild capture images", "update hero image", "update agent pane image", "再生成 画像", "キャプチャ更新", and any request that changes `fixtures/scenarios/**` or the seeding logic in `_lib.sh`.
---

# Regenerate Website Capture Images

The website's hero and feature screenshots (`website/src/assets/captures/*.png`) are generated from real tmux captures driven by `fixtures/scenarios/<name>/scenario.sh`, not hand-edited images. Any visual change must go through the scenario fixtures → rebuild → re-render pipeline. Never edit a capture PNG directly.

## When To Use This Skill

- Changing anything under `fixtures/scenarios/` (scenario scripts, `_lib.sh`, stream files).
- Changing UI code that appears in a capture (row layout, icons, colors, branch label format, activity log rendering, etc.).
- Bumping agent branding (OpenCode icon, Codex label) when the new branding is visible in a capture.

If your change does **not** affect the visible output (pure logic refactor, test-only change, backend-only fix), skip this skill.

## Pipeline Overview

```
fixtures/scenarios/<name>/scenario.sh
  → sources common/_lib.sh (build_layout seeds 4 panes: MAIN, WAITING, BACKGROUND, ERROR)
  → launches real sidebar binary (`target/release/tmux-agent-sidebar`)
  → `capture` subcommand writes <name>.html into a tmp dir
    → scripts/render-frames.mjs (Playwright) converts .html → .png
      → copied to website/src/assets/captures/<name>.png
```

For `hero`, an additional `scripts/hero-compose.mjs` pass produces `og-image.png` that is copied to `website/public/og-image.png`.

## Scenario → Output Map

| Scenario | Output PNG | Crop |
|---|---|---|
| `hero` | `hero.png` (+ `og-image.png`) | full 140×46 canvas |
| `agent-pane-focus` | `agent-pane-focus.png` | rows 0..26, cols 0..46 (agent list) |
| `activity-focus` | `activity-focus.png` | rows 26..46, cols 0..46 (Activity tab) |
| `git-focus` | `git-focus.png` | rows 26..46, cols 0..46 (Git tab) |
| `worktree-spawn` | `worktree-spawn.png` | rows 3..15, cols 0..32 (spawn popup) |

All scenarios share `build_layout` in `_lib.sh`, so a change to the seeded pane state can affect multiple outputs. Before regenerating, decide which PNGs are actually affected (e.g. a port change only shows in the agent list → `hero` + `agent-pane-focus` only; a bottom-tab change → `activity-focus` or `git-focus`).

## Procedure

### 1. Preflight

- Must be on the project root.
- `cargo`, `node`, `tmux` on PATH.
- **`hero` must be run from a `main` checkout** — `build_layout` derives the MAIN pane's branch label from `$ROOT` via git. Running `hero` from a feature branch makes the top row read that branch name instead of `main`.

### 2. Edit the Scenario Source

- Per-pane seeded state (agent, status, branch, port, bg cmd, wait reason, prompt) lives in `build_layout` inside `fixtures/scenarios/common/_lib.sh`.
- Per-scenario composition (which pane is focused, crop region, extra activity log seeding, popup key presses) lives in that scenario's `scenario.sh`.
- `run_fake_agent <pane> <agent> [port] [bg_cmd]` — pass `""` for port to skip the port listener; pass a non-empty `bg_cmd` to spawn a sidecar `sleep` whose `argv[0]` is the label surfaced by ps-based bg detection.

### 3. Build The Release Binary

```bash
cargo build --release
```

Scenarios invoke `target/release/tmux-agent-sidebar`, so a stale debug build will not pick up source changes.

### 4. Render Only The Affected Scenarios

Prefer a targeted loop over `scripts/build-assets.sh` (which re-renders everything, ~2–3 min):

```bash
TMP="$(mktemp -d -t tas-build.XXXX)"
OUT="$PWD/website/src/assets/captures"
for name in hero agent-pane-focus; do         # edit this list
    mkdir -p "$TMP/$name"
    ./fixtures/scenarios/$name/scenario.sh "$TMP/$name"
    ( cd website && node ../scripts/render-frames.mjs "$TMP/$name" )
    cp "$TMP/$name/$name.png" "$OUT/$name.png"
done
```

If `hero` is regenerated, also refresh the og:image:

```bash
( cd website && node ../scripts/hero-compose.mjs \
    "$OUT/hero.png" "$OUT/og-image.png" )
cp "$OUT/og-image.png" website/public/og-image.png
```

Clean up the tmp dir (`rm -rf "$TMP"`).

To regenerate **everything** in one go, `scripts/build-assets.sh` is the single source of truth — prefer it over duplicating its logic when the change affects all scenarios.

### 5. Visual Verification (Mandatory)

Read each regenerated PNG with the `Read` tool and confirm the intended change is visible **and** nothing unrelated regressed:

- Did the change you wanted actually land?
- Did the MAIN pane's branch label read `main` (hero only)?
- Did any row reorder, color shift, or crop-edge artifact appear?
- For `hero`: the terminal chrome (red/yellow/green window dots) and rounded corners must be intact.

Never ship a capture you have not opened and looked at.

### 6. Commit

Commit the scenario fixture change **and** the regenerated PNGs in the same commit so the visual output is always reproducible from `git show`. Do not commit the scenario change alone; do not regenerate without a fixture change.

## Troubleshooting

- **`terminated by signal 9` during scenario run** — a worktree build was copied into the plugin dir without re-signing. This skill builds in the repo itself, so the symlinked plugin dir picks it up for free; only worktrees need the `codesign --force --sign -` step (see `CLAUDE.md` → Debugging).
- **Sidebar shows wrong branch on top row** — you are not on `main`. Check out `main` before running the `hero` scenario.
- **Port not shown next to a pane's branch** — the port-scan pass only finds ports opened by descendants of the fake agent process. Ensure the 3rd arg to `run_fake_agent` is a non-empty number.
- **Bg-command row body missing** — the sidebar requires **both** `@pane_bg_cmd` (seeded via `_seed_pane bg_cmd=…`) and a live process whose `argv[0]` matches. `run_fake_agent …  "" "npm run dev"` seeds both; drop the 4th arg and the body disappears.
- **Rendered PNG is blank or truncated** — the sidebar did not finish its first paint before `capture` ran. `start_sidebar` already sleeps 2s; longer-starting setups may need an extra pause inside the scenario.

## Do Not

- Edit PNGs in an image editor. The pipeline is the source of truth.
- Hand-tune the `.html` snapshot under the tmp dir — regenerate instead.
- Skip the cargo rebuild. A stale binary is the #1 cause of "why did nothing change?".
- Commit PNG-only changes. If the scenario fixtures did not move, neither should the PNGs.
