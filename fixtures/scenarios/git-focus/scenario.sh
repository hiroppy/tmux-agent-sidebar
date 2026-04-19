#!/usr/bin/env bash
# Single-frame snapshot cropped to the Git tab at the bottom.
#
# To demonstrate every Git-tab feature (branch, ahead/behind,
# diff-shortstat, PR number, and all three Staged/Unstaged/Untracked
# sections), the focused pane's cwd is pointed at a purpose-built
# throwaway repo:
#   - feat/login-redirect is 2 commits ahead of origin
#   - origin/feat/login-redirect is 1 commit ahead of the merge base
#   - three staged (A/M/D), three unstaged (M/M/D), three untracked
#   - a stubbed `gh` on PATH that returns PR #42
#
# The repo lives under $TMUX_DIR and is wiped by cleanup.

set -euo pipefail

OUT="${1:?usage: scenario.sh <output-dir> [extra capture args...]}"
shift
EXTRA_ARGS=("$@")

source "$(cd "$(dirname "$0")/../common" && pwd)/_lib.sh"

export FOCUS=PANE_WAITING
# Crop to the sidebar's bottom panel (same as activity-focus).
export CROP_ROWS=26:46
export CROP_COLS=0:46

setup "git-focus"
trap cleanup EXIT

mkdir -p "$OUT"

# ─── fake repo ────────────────────────────────────────────────────
FAKE_REPO="$TMUX_DIR/repo"
mkdir -p "$FAKE_REPO"

# -c flags inline so we don't touch the user's git config, and keep
# commits fast / signatureless regardless of the host's gpg setup.
GC=(-c commit.gpgsign=false -c tag.gpgsign=false -c user.name=tas -c user.email=tas@local)

(
    cd "$FAKE_REPO"
    git "${GC[@]}" init -q -b main
    git "${GC[@]}" commit --allow-empty -q -m "M0: init"
    echo "# login redirect fix" > README.md
    git "${GC[@]}" add README.md
    git "${GC[@]}" commit -q -m "add README"

    # Feature branch: 2 commits ahead of origin.
    git "${GC[@]}" checkout -q -b feat/login-redirect
    mkdir -p src/auth src/middleware
    printf 'export function handleRedirect() {\n  return null\n}\n' > src/auth/redirect.ts
    printf 'export function validateState() {\n  return true\n}\n'  > src/auth/state.ts
    printf 'export function withSession() {}\n'                     > src/auth/session.ts
    printf 'export function logout() {}\n'                          > src/middleware/logout.ts
    git "${GC[@]}" add src/auth src/middleware
    git "${GC[@]}" commit -q -m "scaffold redirect + session"
    printf 'export const NONCE_TTL_MS = 5 * 60 * 1000\n' > src/auth/nonce.ts
    git "${GC[@]}" add src/auth/nonce.ts
    git "${GC[@]}" commit -q -m "nonce TTL constant"

    # Ignore local-only artefacts so they don't show up as untracked
    # and push the showcase files off the visible rows.
    printf 'node_modules/\ndist/\n' > .gitignore
    git "${GC[@]}" add .gitignore
    git "${GC[@]}" commit -q -m "add .gitignore"

    # A divergent commit on main becomes origin/feat/login-redirect so
    # the branch is 1 behind as well. No real remote is needed; the
    # update-ref creates the remote-tracking ref directly.
    git "${GC[@]}" checkout -q main
    echo "docs stub" > CHANGES.md
    git "${GC[@]}" add CHANGES.md
    git "${GC[@]}" commit -q -m "O1: docs"
    ORIGIN_SHA=$(git rev-parse HEAD)
    git "${GC[@]}" checkout -q feat/login-redirect

    # Git refuses to resolve @{upstream} unless the branch's configured
    # remote actually exists in the repo config — even if the remote
    # ref itself is already in place. A placeholder URL (/dev/null)
    # plus the standard fetch refspec is enough to satisfy that.
    git "${GC[@]}" remote add origin /dev/null
    git "${GC[@]}" config remote.origin.fetch '+refs/heads/*:refs/remotes/origin/*'
    git "${GC[@]}" update-ref refs/remotes/origin/feat/login-redirect "$ORIGIN_SHA"
    git "${GC[@]}" config branch.feat/login-redirect.remote origin
    git "${GC[@]}" config branch.feat/login-redirect.merge refs/heads/feat/login-redirect

    # Staged: A (added), M (modified), D (deleted).
    printf '# login redirect fix\n\nPreserve query string on OAuth redirect.\n' > README.md
    git "${GC[@]}" add README.md
    printf 'export const CSP_DIRECTIVES = {}\n' > src/auth/csp.ts
    git "${GC[@]}" add src/auth/csp.ts
    git "${GC[@]}" rm -q src/auth/nonce.ts

    # Unstaged: M × 2 + D × 1 (tracked file removed without staging).
    printf '\n// TODO: preserve query string across the redirect\n' >> src/auth/redirect.ts
    printf '\n// TODO: reject expired nonces\n'                      >> src/auth/state.ts
    rm src/middleware/logout.ts

    # Untracked: a handful of new files, one nested.
    mkdir -p tests docs
    printf "describe('redirect', () => {})\n" > tests/redirect.test.ts
    printf "describe('session', () => {})\n"  > tests/session.test.ts
    echo "# Known issues"                     > NOTES.md
    echo "## Migration notes"                 > docs/MIGRATION.md
)

# ─── fake gh ──────────────────────────────────────────────────────
# The sidebar's git worker runs `gh pr view --json number -q .number`
# and expects stdout to be the PR number. Match the invocation the
# real gh uses and print "42".
cat > "$FAKE_BIN_DIR/gh" <<'GH'
#!/usr/bin/env bash
# Minimal stub: respond to `gh pr view --json number -q .number` with 42.
# Bash's case patterns can't share characters across literal segments,
# so match the full `pr view` phrase in one literal instead of two.
case " $* " in
    *" pr view "*" number "*) echo 42 ;;
    *)                        exit 1 ;;
esac
GH
chmod +x "$FAKE_BIN_DIR/gh"

# Tmux inherits env at server startup, so PATH must be set BEFORE the
# new-session inside build_layout. set-environment -g also forces the
# value onto later panes in case a login shell rewrites PATH.
export PATH="$FAKE_BIN_DIR:$PATH"

build_layout

tmux set-environment -g PATH "$PATH"
paint_stream "$MAIN_PANE" \
    "$ROOT/fixtures/scenarios/hero/main-pane.stream"

# Redirect the focused pane's repo lookup at the fake repo. The git
# worker calls `display-message #{pane_current_path}` (src/tmux.rs
# get_pane_path), so respawn the pane in the fake repo's dir to
# update pane_current_path in place.
tmux respawn-pane -k -t "$PANE_WAITING" -c "$FAKE_REPO" \
    "$FAKE_BIN_DIR/codex 999999"

# Launch the sidebar via respawn-pane with an explicit PATH. The user's
# fish login shell prepends homebrew (where the real `gh` lives) to
# PATH, which otherwise wins over FAKE_BIN_DIR even after `export` +
# `tmux set-environment`. Respawn bypasses the shell, so the fake `gh`
# stub is picked up.
tmux respawn-pane -k -t "$SIDEBAR_PANE" -e "PATH=$PATH" "$BIN"
sleep 2.0

# Switch the bottom panel to the Git tab and give the git worker a
# beat to finish its fetch cycle (shortstat, ahead/behind, gh PR).
tmux send-keys -t "$SIDEBAR_PANE" BTab
sleep 1.5

capture_single
