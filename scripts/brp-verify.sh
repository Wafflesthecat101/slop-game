#!/usr/bin/env bash
# brp-verify.sh — launch the game headlessly with the Bevy Remote Protocol
# (BRP) enabled and assert facts about the *running* ECS world, so a reviewer
# can confirm a change does what it intends at runtime — not just that it
# compiles.
#
# It builds/launches `bevy_game` with `--features review` under the sandbox's
# Xvfb display, waits for the BRP HTTP server (127.0.0.1:15702) to come up,
# then runs a list of assertions. Assertions are read from an "expectations"
# file (one per line) so each PR can declare what to check without editing this
# script. Supported assertion kinds:
#
#   entities <type_path> <op> <n>   # count of entities WITH <type_path> vs n
#   resource_exists <resource_path> # the reflected resource is registered/present
#   resource_absent <resource_path> # the reflected resource is NOT present
#
# where <op> is one of: eq ge le gt lt
# `<type_path>`/`<resource_path>` are fully-qualified Rust reflect paths, e.g.
#   bevy_camera::components::Camera3d
#   bevy_light::point_light::PointLight
# Lines beginning with `#` and blank lines are ignored.
#
# Usage:
#   scripts/brp-verify.sh [expectations_file]
# Defaults to scripts/expectations.default.txt.
#
# Exit code 0 = all assertions passed; non-zero = at least one failed (or the
# app failed to build/launch/expose BRP). Prints a PASS/FAIL line per check.
#
# See docs/REVIEW.md for the full review process this plugs into.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

EXPECT_FILE="${1:-scripts/expectations.default.txt}"
BRP_URL="http://127.0.0.1:15702"
DISPLAY_NUM=":97"
START_DISPLAY="$HOME/workspace/start_bevy_display.sh"
BOOT_SECONDS="${BRP_BOOT_SECONDS:-8}"    # give the world a few frames to spawn
GAME_LOG="$(mktemp -t brp-game.XXXXXX.log)"
GAME_PID=""

log()  { printf '%s\n' "$*" >&2; }
pass() { printf 'PASS  %s\n' "$*"; }
fail() { printf 'FAIL  %s\n' "$*"; FAILURES=$((FAILURES + 1)); }

cleanup() {
    if [ -n "$GAME_PID" ] && kill -0 "$GAME_PID" 2>/dev/null; then
        kill "$GAME_PID" 2>/dev/null
        wait "$GAME_PID" 2>/dev/null
    fi
}
trap cleanup EXIT INT TERM

# --- POST a BRP JSON-RPC request; echoes the raw JSON response. -------------
brp() {
    local method="$1" params="${2:-null}"
    curl -s -m 10 -X POST "$BRP_URL" -H 'Content-Type: application/json' \
        -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$method\",\"params\":$params}"
}

# --- Count entities that have a given component type path. ------------------
entity_count() {
    local type_path="$1"
    brp world.query "{\"data\":{\"has\":[]},\"filter\":{\"with\":[\"$type_path\"]}}" \
        | jq 'if .error then "err" else (.result | length) end'
}

# --- Compare two integers with a named operator. ----------------------------
cmp_int() {
    local a="$1" op="$2" b="$3"
    case "$op" in
        eq) [ "$a" -eq "$b" ] ;;
        ge) [ "$a" -ge "$b" ] ;;
        le) [ "$a" -le "$b" ] ;;
        gt) [ "$a" -gt "$b" ] ;;
        lt) [ "$a" -lt "$b" ] ;;
        *)  return 2 ;;
    esac
}

# --- 1. Build with the review feature. --------------------------------------
log "==> Building bevy_game --features review (first run compiles bevy_remote)…"
if ! cargo build -p bevy_game --features review >/dev/null 2>"$GAME_LOG.build"; then
    log "Build failed:"; grep -iE 'error' "$GAME_LOG.build" | head >&2
    echo "FAIL  build --features review"
    exit 1
fi

# Bevy resolves assets/ relative to CWD, not the exe.
ln -sfn ../../assets target/debug/assets 2>/dev/null || true

# --- 2. Bring up the headless display and launch the game. ------------------
if [ -x "$START_DISPLAY" ]; then bash "$START_DISPLAY" >/dev/null 2>&1 || true; fi
export DISPLAY="$DISPLAY_NUM"
export XDG_RUNTIME_DIR="/tmp/bevy-xdg-runtime-$(id -u)"
mkdir -p "$XDG_RUNTIME_DIR" && chmod 700 "$XDG_RUNTIME_DIR"

log "==> Launching game with BRP…"
RUST_LOG=warn ./target/debug/bevy_game >"$GAME_LOG" 2>&1 &
GAME_PID=$!

# --- 3. Wait for BRP to answer (and for the game to survive boot). ----------
up=0
for _ in $(seq 1 30); do
    if ! kill -0 "$GAME_PID" 2>/dev/null; then
        log "Game process exited during boot. Last log lines:"; tail -15 "$GAME_LOG" >&2
        echo "FAIL  game booted (process died)"
        exit 1
    fi
    if curl -s -m 2 -o /dev/null -X POST "$BRP_URL" \
        -H 'Content-Type: application/json' \
        -d '{"jsonrpc":"2.0","id":1,"method":"rpc.discover"}'; then
        up=1; break
    fi
    sleep 1
done
if [ "$up" -ne 1 ]; then
    log "BRP endpoint never came up. Last log lines:"; tail -15 "$GAME_LOG" >&2
    echo "FAIL  BRP endpoint reachable"
    exit 1
fi
pass "game booted and BRP endpoint reachable"

# Panic check: a running BRP doesn't prove the sim is healthy on its own.
if grep -qiE 'panicked|thread .* panicked' "$GAME_LOG"; then
    log "Panic detected in game log:"; grep -iE 'panic' "$GAME_LOG" | head >&2
    echo "FAIL  no panic during run"
    exit 1
fi
pass "no panic during run"

# Let the world settle so spawn systems have run.
sleep "$BOOT_SECONDS"

# --- 4. Run the declared expectations. --------------------------------------
if [ ! -f "$EXPECT_FILE" ]; then
    log "No expectations file at '$EXPECT_FILE'; ran smoke + BRP liveness only."
    exit 0
fi

FAILURES=0
log "==> Checking expectations from $EXPECT_FILE"
while read -r kind a b c _rest; do
    case "$kind" in
        ''|\#*) continue ;;
        entities)
            got="$(entity_count "$a")"
            if [ "$got" = "err" ] || [ "$got" = "null" ]; then
                fail "entities $a $b $c (query error — is the type path registered?)"
            elif cmp_int "$got" "$b" "$c"; then
                pass "entities $a $b $c (got $got)"
            else
                fail "entities $a $b $c (got $got)"
            fi
            ;;
        resource_exists)
            if brp world.get_resources "{\"resource\":\"$a\"}" | jq -e '.result != null and (.error|not)' >/dev/null; then
                pass "resource_exists $a"
            else
                fail "resource_exists $a (unknown/absent — needs #[derive(Reflect)] + register_type)"
            fi
            ;;
        resource_absent)
            if brp world.get_resources "{\"resource\":\"$a\"}" | jq -e '.error != null' >/dev/null; then
                pass "resource_absent $a"
            else
                fail "resource_absent $a (still present)"
            fi
            ;;
        *)
            fail "unknown assertion kind '$kind' (line: $kind $a $b $c)"
            ;;
    esac
done < "$EXPECT_FILE"

log ""
if [ "$FAILURES" -eq 0 ]; then
    log "All BRP expectations passed."
    exit 0
else
    log "$FAILURES BRP expectation(s) failed."
    exit 1
fi
