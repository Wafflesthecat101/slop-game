#!/usr/bin/env bash
# walkthrough.sh — drive the running game over BRP and capture screenshots, so
# a reviewer can *see* what a PR looks like in-world (not just assert numbers).
#
# Requires the game running with `--features review` (which pulls in
# bevy_brp_extras, providing brp_extras/screenshot + brp_extras/send_keys).
# `scripts/brp-verify.sh` and `scripts/review-pr.sh` build/launch that for you;
# or run it yourself: `cargo run --features review`.
#
# It captures an opening frame, then walks the player forward/sideways with
# simulated key presses (through the real player controller, so terrain
# following, sprint, etc. all apply), screenshotting after each leg. Frames are
# written to an output dir as PNGs rendered by Bevy itself — works headlessly
# under the sandbox's software renderer, no external screenshot tool needed.
#
# Usage:
#   scripts/walkthrough.sh [out_dir]        # default: ./walkthrough/
# Env:
#   BRP_URL   (default http://127.0.0.1:15702)
#   STEPS     (space-separated "keys:ms" legs; keys are '+'-joined KeyCodes)
#             default: "KeyW:1500 KeyD:800 KeyW+ShiftLeft:1400 KeyA:800 KeyW:1200"
set -uo pipefail

OUT_DIR="${1:-walkthrough}"
BRP_URL="${BRP_URL:-http://127.0.0.1:15702}"
STEPS="${STEPS:-KeyW:1500 KeyD:800 KeyW+ShiftLeft:1400 KeyA:800 KeyW:1200}"
mkdir -p "$OUT_DIR"

brp() { # method, params-json
    curl -s -m 30 -X POST "$BRP_URL" -H 'Content-Type: application/json' \
        -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$1\",\"params\":${2:-null}}"
}

# Confirm the extras screenshot method is available before we start.
if ! brp rpc.discover | jq -e '.result.methods[].name | select(. == "brp_extras/screenshot")' >/dev/null; then
    echo "brp_extras/screenshot not found — is the game running with --features review?" >&2
    exit 1
fi

shoot() { # index, label
    local path; path="$(cd "$OUT_DIR" && pwd)/$(printf '%02d' "$1")_$2.png"
    brp brp_extras/screenshot "{\"path\":\"$path\"}" \
        | jq -e '.result.status == "completed"' >/dev/null \
        && echo "captured $path" || echo "screenshot failed at step $1" >&2
}

# Camera translation, for a movement log alongside the frames.
cam_pos() {
    local ent
    ent="$(brp world.query '{"data":{"has":[]},"filter":{"with":["bevy_camera::components::Camera3d"]}}' | jq '.result[0].entity')"
    brp world.get_components \
        "{\"entity\":$ent,\"components\":[\"bevy_transform::components::transform::Transform\"]}" \
        | jq -c '.result.components["bevy_transform::components::transform::Transform"].translation'
}

i=0
echo "pos $(cam_pos) — opening frame"
shoot "$i" start

for step in $STEPS; do
    i=$((i + 1))
    keys="${step%%:*}"; ms="${step##*:}"
    # "KeyW+ShiftLeft" -> ["KeyW","ShiftLeft"]
    keys_json="$(printf '%s' "$keys" | jq -R 'split("+")')"
    echo "leg $i: hold [$keys] ${ms}ms"
    brp brp_extras/send_keys "{\"keys\":$keys_json,\"duration_ms\":$ms}" \
        | jq -e '.result.success == true' >/dev/null || echo "send_keys failed (leg $i)" >&2
    sleep "$(awk "BEGIN{print $ms/1000 + 0.5}")"
    echo "pos $(cam_pos)"
    shoot "$i" "$(printf '%s' "$keys" | tr '+' '-')"
done

echo "Walkthrough frames written to $OUT_DIR/"
