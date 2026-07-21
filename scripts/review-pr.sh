#!/usr/bin/env bash
# review-pr.sh — verify that a pull request builds, passes the gate, and does
# what it intends at runtime, using Bevy tooling (headless run + Bevy Remote
# Protocol introspection).
#
# The review runs in layers, cheapest first, and stops at the first failing
# layer so feedback is fast:
#
#   Layer 1  Static gate      cargo fmt --check, clippy -D warnings,
#                             test --lib, wasm check       (from CONTRIBUTING.md)
#   Layer 2  Headless smoke   launch under Xvfb, confirm it boots & runs a few
#                             seconds with no panic/ERROR   (via brp-verify.sh)
#   Layer 3  Runtime intent   query the live ECS over BRP and assert the PR's
#                             intended world state          (via brp-verify.sh)
#
# Usage:
#   scripts/review-pr.sh <pr_number> [expectations_file]
#   scripts/review-pr.sh --branch <branch> [expectations_file]
#   scripts/review-pr.sh --worktree [expectations_file]   # review the current tree
#
# Fetching a PR uses the GitHub API (curl) so it works without the `gh` CLI;
# set GITHUB_TOKEN for private repos / higher rate limits. OWNER/REPO are read
# from the `origin` remote.
#
# Exit code 0 only if every layer passes. A concise per-layer PASS/FAIL summary
# is printed at the end (suitable for pasting into a PR review comment).
#
# See docs/REVIEW.md for the rationale and how to extend the expectations.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

SCRIPT_DIR="$REPO_ROOT/scripts"
MODE="pr"
TARGET=""
EXPECT_FILE=""

usage() { sed -n '2,32p' "$0"; exit 2; }

case "${1:-}" in
    --branch)   MODE="branch";   TARGET="${2:-}"; EXPECT_FILE="${3:-}"; [ -n "$TARGET" ] || usage ;;
    --worktree) MODE="worktree"; EXPECT_FILE="${2:-}" ;;
    ''|-h|--help) usage ;;
    *)          MODE="pr";       TARGET="$1";      EXPECT_FILE="${2:-}" ;;
esac
EXPECT_FILE="${EXPECT_FILE:-$SCRIPT_DIR/expectations.default.txt}"

log()     { printf '%s\n' "$*" >&2; }
section() { log ""; log "════════════════════════════════════════════════════════════"; log "  $*"; log "════════════════════════════════════════════════════════════"; }

ORIGINAL_REF="$(git rev-parse --abbrev-ref HEAD)"
CHECKED_OUT=""
restore() {
    if [ -n "$CHECKED_OUT" ]; then
        git checkout -q "$ORIGINAL_REF" 2>/dev/null || true
        git branch -D "$CHECKED_OUT" >/dev/null 2>&1 || true
    fi
}
trap restore EXIT INT TERM

# --- Resolve OWNER/REPO from the origin remote. -----------------------------
origin_slug() {
    git remote get-url origin 2>/dev/null \
        | sed -E 's#(git@|https://)[^/:]+[/:]##; s#\.git$##'
}

# --- Check out the code under review (unless reviewing the working tree). ----
checkout_target() {
    case "$MODE" in
        worktree)
            log "Reviewing the current working tree ($ORIGINAL_REF)."
            ;;
        branch)
            log "Checking out branch '$TARGET'…"
            git fetch -q origin "$TARGET"
            CHECKED_OUT="review/$TARGET"
            git checkout -q -B "$CHECKED_OUT" "origin/$TARGET"
            ;;
        pr)
            local slug; slug="$(origin_slug)"
            log "Fetching PR #$TARGET from $slug via GitHub API…"
            local auth=(); [ -n "${GITHUB_TOKEN:-}" ] && auth=(-H "Authorization: Bearer $GITHUB_TOKEN")
            local head_ref
            head_ref="$(curl -s "${auth[@]}" \
                "https://api.github.com/repos/$slug/pulls/$TARGET" \
                | jq -r '.head.ref // empty')"
            if [ -z "$head_ref" ]; then
                log "Could not resolve PR #$TARGET head branch (check number/token)."
                exit 1
            fi
            log "PR #$TARGET head branch: $head_ref"
            # `refs/pull/N/head` works even for forks.
            git fetch -q origin "pull/$TARGET/head"
            CHECKED_OUT="review/pr-$TARGET"
            git checkout -q -B "$CHECKED_OUT" FETCH_HEAD
            ;;
    esac
}

# ---------------------------------------------------------------------------
declare -A RESULT
run_layer() { # name, command...
    local name="$1"; shift
    if "$@"; then RESULT[$name]="PASS"; else RESULT[$name]="FAIL"; return 1; fi
}

section "Preparing"
checkout_target
log "HEAD is now $(git rev-parse --short HEAD) ($(git log -1 --pretty=%s))"

OVERALL=0

section "Layer 1 — Static gate (fmt / clippy / test / wasm)"
gate() {
    cargo fmt --check \
        && cargo clippy -p bevy_game --all-targets -- -D warnings \
        && cargo test -p bevy_game --lib \
        && cargo check -p bevy_game --target wasm32-unknown-unknown
}
run_layer gate gate || OVERALL=1

if [ "${RESULT[gate]}" = "PASS" ]; then
    section "Layer 2+3 — Headless run + BRP runtime intent checks"
    run_layer runtime bash "$SCRIPT_DIR/brp-verify.sh" "$EXPECT_FILE" || OVERALL=1
else
    log "Skipping runtime layers because the static gate failed."
    RESULT[runtime]="SKIP"
fi

# --- Summary (paste-ready for a PR review comment). -------------------------
section "Review summary"
printf '%s\n' "| Layer | Result |"
printf '%s\n' "|-------|--------|"
printf '| Static gate (fmt/clippy/test/wasm) | %s |\n' "${RESULT[gate]:-SKIP}"
printf '| Headless run + BRP runtime intent  | %s |\n' "${RESULT[runtime]:-SKIP}"
printf '\n'
if [ "$OVERALL" -eq 0 ]; then
    printf 'OVERALL: PASS — builds, gate is green, boots headlessly, and the live ECS matches the expected world state.\n'
else
    printf 'OVERALL: FAIL — see the failing layer above.\n'
fi
exit "$OVERALL"
