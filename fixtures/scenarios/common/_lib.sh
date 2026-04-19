# Shared shell library sourced by every scenario.sh.
#
# Provides:
#   setup <session-name>  — prepare tmux server, create session, init vars
#   cleanup               — kill the isolated tmux server, clean tmp files
#   build_layout          — create the 4-pane hero layout with metadata
#   paint_stream <pane> <stream-file> [agent]
#                         — cat a .stream file into a pane, keeping a fake
#                           agent process alive afterwards
#   run_fake_agent <pane> <agent> [port]
#                         — respawn a pane with just the fake agent binary
#                           (and optional TCP listener on `port`)
#   start_sidebar         — launch the real sidebar binary in $SIDEBAR_PANE
#   capture_loop <ms> <fps>
#                         — call `capture --frames-out …` for a loop
#   capture_single        — call `capture --frame-out $OUT/<name>.html`
#
# Per-scenario env vars (optional, set before build_layout):
#   FOCUS=PANE_WAITING|PANE_RUNNING_2|PANE_ERROR|MAIN_PANE
#         which agent pane is the "focused" one. Default MAIN_PANE.
#   BOTTOM_HEIGHT=N — override @sidebar_bottom_height (0 hides the panel).
#   CROP_ROWS=N:M  — vertical crop applied to capture_single output.
#   CROP_COLS=N:M  — horizontal crop applied to capture_single output.
#
# Every function expects to be called from within a scenario.sh so that
# ${BASH_SOURCE[1]} resolves to that scenario's path; `setup` walks up
# three directories to find the project root.

# -- initialisation -------------------------------------------------

setup() {
    local session_name="$1"

    local caller="${BASH_SOURCE[1]-}"
    if [[ -n "$caller" ]]; then
        ROOT="$(cd "$(dirname "$caller")/../../.." && pwd)"
    else
        ROOT="$PWD"
    fi
    SESSION="$session_name"
    BIN="$ROOT/target/release/tmux-agent-sidebar"
    TMUX_CONF="$ROOT/fixtures/scenarios/common/.tmux.conf"

    # CRITICAL: if the scenario is invoked from inside a tmux session,
    # the shell has TMUX set to the *user's* socket. Every tmux command
    # we run — including `kill-server` in cleanup — would then target
    # the user's real server and destroy their work.
    #
    # Unset TMUX (and TMUX_PANE) FIRST, then redirect TMUX_TMPDIR to a
    # per-run directory. Subsequent tmux commands land on
    # $TMUX_TMPDIR/tmux-<uid>/default — an isolated socket that can't
    # reach the user's real server.
    unset TMUX TMUX_PANE

    # Path length matters: macOS Unix sockets cap at ~104 bytes. Keep
    # TMUX_TMPDIR short. `/tmp/tas.` prefix is our cleanup guard sentinel.
    TMUX_DIR="$(mktemp -d /tmp/tas.XXXXXX)"
    export TMUX_TMPDIR="$TMUX_DIR"

    # Clean slate for activity logs that any prior run may have left.
    rm -f /tmp/tmux-agent-activity_*.log

    # The sidebar's port-scan pass clears @pane_* on any pane whose
    # process tree does not contain a real `claude` or `codex`
    # executable. To keep our seeded metadata alive, we symlink the
    # real /bin/sleep under names `claude` and `codex` and run those
    # in each agent pane.
    #
    # Symlinks (not copies) because macOS System Integrity Protection
    # SIGKILLs plain copies of system binaries — the ad-hoc linker
    # signature of the copy isn't trusted by the kernel.
    FAKE_BIN_DIR="$TMUX_DIR/bin"
    mkdir -p "$FAKE_BIN_DIR"
    ln -sf /bin/sleep "$FAKE_BIN_DIR/claude"
    ln -sf /bin/sleep "$FAKE_BIN_DIR/codex"
    export FAKE_BIN_DIR
}

cleanup() {
    # Only touch servers under our own TMUX_TMPDIR prefix, as a
    # defence against running without setup (where TMUX_TMPDIR could
    # still point at the user's default socket).
    if [[ -n "${TMUX_TMPDIR:-}" && "${TMUX_TMPDIR:-}" == /tmp/tas.* ]]; then
        tmux kill-server 2>/dev/null || true
    fi
    rm -rf "${TMUX_DIR:-}"
    rm -f /tmp/tmux-agent-activity_*.log
}

# -- layout ---------------------------------------------------------

# Creates the hero's 4-pane layout across two tmux windows:
#   - Window 0 (captured): sidebar pane (narrow, left) + main agent pane
#     (wide, right).
#   - Window 1 (off-screen): 3 extra agent panes.
# `list-panes -a` enumerates all four agents in the sidebar's list
# without cluttering the captured window.
#
# Exports:
#   WINDOW, SIDEBAR_PANE, MAIN_PANE, MAIN_LOG,
#   PANE_WAITING, PANE_RUNNING_2, PANE_ERROR,
#   FOCUSED_PANE, FOCUSED_LOG
build_layout() {
    # Canvas 140×46 — width ≈ 1220 px, height ≈ 815 px. Fits a 1280×900
    # viewport with room for the mac-window shadow.
    tmux -f "$TMUX_CONF" new-session -d -s "$SESSION" -x 140 -y 46 -c "$ROOT"

    # Reset persisted state so every run renders predictably.
    tmux set-option -g @sidebar_filter all
    if [[ -n "${BOTTOM_HEIGHT:-}" ]]; then
        tmux set-option -g @sidebar_bottom_height "$BOTTOM_HEIGHT"
    fi

    export WINDOW
    WINDOW=$(tmux display-message -t "$SESSION" -p '#{window_id}')
    export SIDEBAR_PANE
    SIDEBAR_PANE=$(tmux list-panes -t "$WINDOW" -F '#{pane_id}' | head -n 1)
    tmux set-option -t "$SIDEBAR_PANE" -p @pane_role sidebar

    # 33/67 horizontal split at 140 cols: sidebar ~46 cols (just above
    # the ~45-col pane-list rendering minimum), agent pane ~93 cols.
    # `split-window -P -F #{pane_id}` returns the new pane's id directly,
    # saving a second list-panes round-trip.
    export MAIN_PANE
    MAIN_PANE=$(tmux split-window -h -t "$SIDEBAR_PANE" -p 67 -c "$ROOT" -P -F '#{pane_id}')

    # Main pane: Claude on `main` investigating a CI flake. Non-worktree
    # branch at the top of the list.
    _seed_pane "$MAIN_PANE" \
        agent=claude status=running \
        branch=main \
        prompt="investigate the nightly CI flake in the fixtures runner" \
        started_s=423
    tmux set-option -t "$MAIN_PANE" -p @pane_subagents \
        "Explore:a1b2c3de,Plan:d4e5f6ab,Bash:deadbeef"

    export MAIN_LOG="/tmp/tmux-agent-activity${MAIN_PANE/\%/_}.log"
    : > "$MAIN_LOG"

    # Off-screen window hosting the 3 other agents. Idle panes are
    # intentionally omitted — in real use, an agent going idle means
    # its session ended and clear_dead_agent_metadata removes it from
    # the list. Keeping one in the hero would be misleading.
    local extra_win
    extra_win=$(tmux new-window -d -t "$SESSION" -n extras -c "$ROOT" -P -F '#{window_id}')
    tmux split-window -h -t "$extra_win" -c "$ROOT"
    tmux split-window -v -t "$extra_win" -c "$ROOT"

    # Portable alternative to `mapfile -t` (bash 3.2 on macOS lacks it).
    extras=()
    while IFS= read -r line; do
        extras+=("$line")
    done < <(tmux list-panes -t "$extra_win" -F '#{pane_id}')

    # Waiting pane — Codex just hit a permission prompt 45 s ago.
    export PANE_WAITING="${extras[0]}"
    _seed_pane "$PANE_WAITING" \
        agent=codex status=waiting attention=notification \
        branch=fix/login-redirect \
        prompt="deep-link query string dropped on redirect" \
        wait_reason=permission_required \
        started_s=45
    run_fake_agent "$PANE_WAITING" codex

    # Second Claude running — long-lived refactor (12 m 45 s).
    export PANE_RUNNING_2="${extras[1]}"
    _seed_pane "$PANE_RUNNING_2" \
        agent=claude status=running \
        branch=refactor/api-layer \
        prompt="split the monolithic handler into typed routers" \
        started_s=765
    run_fake_agent "$PANE_RUNNING_2" claude

    # Error pane — cause surfaced via @pane_wait_reason (matches the hook
    # convention in src/cli/hook/handlers.rs). Listens on :3456 — the
    # local preview that was already up when the agent hit the rate limit.
    # _seed_pane skips @pane_started_at on error so the elapsed counter
    # stays empty; the failure reason matters, not how old the error is.
    export PANE_ERROR="${extras[2]}"
    _seed_pane "$PANE_ERROR" \
        agent=claude status=error \
        branch=feat/dashboard-charts \
        prompt="add Recharts integration to the metrics panel" \
        wait_reason="anthropic api: rate limit reached"
    run_fake_agent "$PANE_ERROR" claude 3456

    # Default focus = MAIN_PANE. Scenarios override via FOCUS.
    # `${!var}` indirection under `set -u` fails hard if FOCUS points at
    # an unset variable, so collect the known pane names through a case
    # rather than trusting the caller's FOCUS string unconditionally.
    local focus_pane="$MAIN_PANE"
    case "${FOCUS:-MAIN_PANE}" in
        SIDEBAR_PANE)    focus_pane="$SIDEBAR_PANE" ;;
        MAIN_PANE)       focus_pane="$MAIN_PANE" ;;
        PANE_WAITING)    focus_pane="$PANE_WAITING" ;;
        PANE_RUNNING_2) focus_pane="$PANE_RUNNING_2" ;;
        PANE_ERROR)      focus_pane="$PANE_ERROR" ;;
        *) echo "build_layout: unknown FOCUS=${FOCUS}" >&2; return 1 ;;
    esac

    # has_focus requires BOTH window_active=1 AND pane_active=1. If the
    # focused pane lives in the extras window, activate that window
    # first; the captured window is pinned separately via the capture
    # functions' --window flag. MAIN_PANE lives in $WINDOW, the three
    # extras live in $extra_win — no need to query tmux for the mapping.
    local focus_window="$WINDOW"
    case "$focus_pane" in
        "$MAIN_PANE"|"$SIDEBAR_PANE") focus_window="$WINDOW" ;;
        *) focus_window="$extra_win" ;;
    esac
    tmux select-window -t "$focus_window"
    tmux select-pane -t "$focus_pane"

    export FOCUSED_PANE="$focus_pane"
    export FOCUSED_LOG="/tmp/tmux-agent-activity${focus_pane/\%/_}.log"
    : > "$FOCUSED_LOG"
}

# _seed_pane <pane_id> key=value ...
# Supported keys: agent, status, attention, branch,
#                 prompt, wait_reason, started_s (seconds ago; default 420)
_seed_pane() {
    local p="$1"
    shift

    local agent="" status="" attention="" branch="" prompt="" wait_reason="" started_s=420
    local kv
    for kv in "$@"; do
        case "$kv" in
            agent=*)       agent="${kv#agent=}" ;;
            status=*)      status="${kv#status=}" ;;
            attention=*)   attention="${kv#attention=}" ;;
            branch=*)      branch="${kv#branch=}" ;;
            prompt=*)      prompt="${kv#prompt=}" ;;
            wait_reason=*) wait_reason="${kv#wait_reason=}" ;;
            started_s=*)   started_s="${kv#started_s=}" ;;
            *) echo "_seed_pane: unknown kv: $kv" >&2; return 1 ;;
        esac
    done

    tmux set-option -t "$p" -p @pane_agent           "$agent"
    tmux set-option -t "$p" -p @pane_status          "$status"
    tmux set-option -t "$p" -p @pane_worktree_branch "$branch"
    # @pane_worktree_name deliberately left unset. branch_label() in
    # src/ui/text.rs renders "+ <branch>" when the worktree name is
    # empty and "+ <name>: <branch>" when it's set.
    tmux set-option -t "$p" -p @pane_prompt          "$prompt"
    tmux set-option -t "$p" -p @pane_prompt_source   user
    if [[ "$status" != "error" ]]; then
        tmux set-option -t "$p" -p @pane_started_at "$(($(date +%s) - started_s))"
    fi
    if [[ -n "$attention" ]]; then
        tmux set-option -t "$p" -p @pane_attention "$attention"
    fi
    if [[ -n "$wait_reason" ]]; then
        tmux set-option -t "$p" -p @pane_wait_reason "$wait_reason"
    fi
}

# -- stream painting ------------------------------------------------

# Paint a pane with a .stream file, then replace its process with the
# fake agent so the sidebar's liveness check stays satisfied. `tmux
# respawn-pane -k` swaps the process directly rather than through the
# shell's command prompt — more reliable than `send-keys "exec …"`
# when the default shell hasn't reached its prompt yet. Escape
# sequences in the stream use literal `\e`; an awk pass converts them
# to real ESC bytes before `cat` emits them.
paint_stream() {
    local pane="$1" stream="$2" agent="${3:-claude}"
    local painted="$TMUX_DIR/$(basename "$stream").painted"
    awk '{ gsub(/\\e/, "\033"); print }' < "$stream" > "$painted"
    tmux respawn-pane -k -t "$pane" \
        "sh -c 'cat \"$painted\"; exec \"$FAKE_BIN_DIR/$agent\" 999999'"
}

# Respawn a pane with just the fake agent binary. Optional 3rd arg is
# a TCP port; when set, the pane runs both the fake agent and a minimal
# Python http.server in the background under a shared sh parent, so
# the port-scan pass reports the listening port next to this pane's
# branch label.
run_fake_agent() {
    local pane="$1" agent="$2" port="${3:-}"
    if [[ -n "$port" ]]; then
        tmux respawn-pane -k -t "$pane" \
            "sh -c '$FAKE_BIN_DIR/$agent 999999 & python3 -m http.server $port >/dev/null 2>&1 & wait'"
    else
        tmux respawn-pane -k -t "$pane" "$FAKE_BIN_DIR/$agent 999999"
    fi
}

# -- sidebar + capture ---------------------------------------------

# Start the real sidebar binary in $SIDEBAR_PANE and wait for the
# first frame. 2 s is enough for the port-scan cycle to identify
# every agent and for the TUI to paint its first full frame.
start_sidebar() {
    tmux send-keys -t "$SIDEBAR_PANE" "$BIN" Enter
    sleep 2.0
}

# Run a frame-sequence capture (for loops). `default_ms` / `default_fps`
# are used when the caller didn't pass their own via $EXTRA_ARGS.
capture_loop() {
    local default_ms="$1" default_fps="$2"
    local args=("${EXTRA_ARGS[@]}")
    if [[ ${#args[@]} -eq 0 ]]; then
        args=(--duration-ms "$default_ms" --fps "$default_fps")
    fi
    "$BIN" capture \
        --session "$SESSION" \
        --window "$WINDOW" \
        --frames-out "$OUT" \
        "${args[@]}"
}

# Single-frame capture. Output goes to $OUT/<SESSION>.html. Scenarios
# can set CROP_ROWS=N:M and/or CROP_COLS=N:M (END exclusive) to trim
# the rendered grid before it becomes HTML — e.g. just the Activity
# tab, just a popup, etc.
capture_single() {
    local crop_args=()
    if [[ -n "${CROP_ROWS:-}" ]]; then
        crop_args+=(--crop-rows "$CROP_ROWS")
    fi
    if [[ -n "${CROP_COLS:-}" ]]; then
        crop_args+=(--crop-cols "$CROP_COLS")
    fi

    if [[ ${#EXTRA_ARGS[@]} -ne 0 ]]; then
        # Caller override: treat as a frames-sequence capture instead.
        "$BIN" capture \
            --session "$SESSION" \
            --window "$WINDOW" \
            --frames-out "$OUT" \
            ${crop_args[@]+"${crop_args[@]}"} \
            "${EXTRA_ARGS[@]}"
    else
        "$BIN" capture \
            --session "$SESSION" \
            --window "$WINDOW" \
            --frame-out "$OUT/$SESSION.html" \
            ${crop_args[@]+"${crop_args[@]}"}
    fi
}
