# Reviewing a PR — verify it works, not just that it compiles

Lumen is built by many contributors (including agents) in parallel, so a PR
needs to be checked quickly and objectively. This repo ships a **layered,
scriptable review process** that uses Bevy's own tooling to confirm a change
**does what it intends at runtime** — it boots the game headlessly and
introspects the live ECS world over the **Bevy Remote Protocol (BRP)** — on top
of the usual static gate.

> TL;DR: `scripts/review-pr.sh <pr_number>` and read the PASS/FAIL summary.

## Why runtime verification

`cargo build`/`clippy`/`test` prove the code *compiles and its unit tests pass*.
They don't prove the game still *spawns a player and 12 shrines*, still lights
the scene, or that a new plugin actually took effect. Because this is a Bevy
app we can check that directly: run it and ask the running `World` what's in it.

## The layers

The pipeline runs cheapest-first and stops at the first failing layer:

| Layer | What | Tool |
|-------|------|------|
| 1. Static gate | `fmt --check`, `clippy -D warnings`, `test --lib`, wasm `check` | cargo (see [`CONTRIBUTING.md`](CONTRIBUTING.md)) |
| 2. Headless smoke | launches under Xvfb; must boot and run a few seconds with **no panic** and a reachable BRP endpoint | `scripts/brp-verify.sh` |
| 3. Runtime intent | queries the **live ECS** over BRP and asserts the world matches expectations (entity counts, resources present/absent) | `scripts/brp-verify.sh` + an expectations file |

## The `review` cargo feature (BRP)

Layers 2–3 need the game to expose BRP. That is gated behind an **opt-in
`review` feature** so the shipped game (native + WASM) never carries it:

```toml
# Cargo.toml
[features]
review = ["bevy/bevy_remote"]
```

```rust
// src/lib.rs — inside GamePlugin::build
#[cfg(feature = "review")]
app.add_plugins((
    bevy::remote::RemotePlugin::default(),
    bevy::remote::http::RemoteHttpPlugin::default(), // 127.0.0.1:15702
));
```

Build/run it yourself with `cargo run --features review`, then talk to it with
`curl`, [`bevy_brp_mcp`](https://github.com/natepiano/bevy_brp), or any BRP
client. This bevy_remote (0.19) uses the `world.*` JSON-RPC method namespace
(`world.query`, `world.list_resources`, `world.get_resources`, `rpc.discover`, …).

The `review` feature also pulls in
[`bevy_brp_extras`](https://crates.io/crates/bevy_brp_extras), which adds
`brp_extras/*` methods on top of core BRP — most usefully **in-engine viewport
screenshots** (`brp_extras/screenshot`) and **input simulation**
(`brp_extras/send_keys`). Because the frame is rendered by Bevy itself, this
works under the sandbox's software renderer with no external screenshot tool.

## Seeing the world — screenshots & walkthroughs

Numbers prove structure; screenshots prove it *looks* right. With the game
running under `--features review`:

```bash
# One frame from the player camera:
curl -s -X POST http://127.0.0.1:15702 -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"brp_extras/screenshot","params":{"path":"/tmp/shot.png"}}'

# Walk the player and capture frames along the way:
scripts/walkthrough.sh out/            # writes 00_start.png, 01_KeyW.png, …
```

`scripts/walkthrough.sh` drives the *real* player controller with simulated key
presses (`brp_extras/send_keys` — so terrain-following, sprint FOV, collision,
etc. all apply), logs the camera position after each leg, and screenshots. Set
`STEPS` to script a custom path, e.g.
`STEPS="KeyW:2000 KeyD:1000 KeyW+ShiftLeft:1500" scripts/walkthrough.sh`.
Key names are Bevy `KeyCode` variants (`KeyW`, `ShiftLeft`, `Space`, …).

## Running the review

```bash
# Review an open PR by number (uses the GitHub API; no `gh` CLI needed).
# Set GITHUB_TOKEN for private repos / higher rate limits.
scripts/review-pr.sh 41

# Review a remote branch, or just the current working tree:
scripts/review-pr.sh --branch feat/game-state-machine
scripts/review-pr.sh --worktree
```

The script checks out the PR into a throwaway `review/…` branch, runs the
layers, restores your original branch, and prints a paste-ready Markdown
summary for the PR comment.

### Just the runtime checks

```bash
scripts/brp-verify.sh                       # uses scripts/expectations.default.txt
scripts/brp-verify.sh scripts/expectations.my-pr.txt
BRP_BOOT_SECONDS=10 scripts/brp-verify.sh   # wait longer before asserting
```

## Expectations files — telling the reviewer the *intent*

An expectations file declares, one assertion per line, what the running world
must look like. Blank lines and `#` comments are ignored.

```
entities <fully::qualified::TypePath> <eq|ge|le|gt|lt> <n>
resource_exists <fully::qualified::ResourcePath>
resource_absent  <fully::qualified::ResourcePath>
```

`scripts/expectations.default.txt` holds the baseline invariants for `main`
(one `Camera3d`, twelve beacon `PointLight`s, a sun, sky + ambient resources).

**When your PR changes gameplay, ship its intent as an expectations file.** Copy
the default, adjust it, and point the reviewer at it. Examples:

- *Day/night cycle (#11)* — assert `entities …DirectionalLight ge 1` and that the
  sun's transform/colour changes over time (extend the script with a
  before/after `world.get_components` read).
- *A new plugin that spawns N objects* — `entities <your::Marker> eq N`.
- *Removing an entity kind* — `entities <old::Marker> eq 0`.

### Making your own types visible to BRP

BRP can only see types that are **reflected and registered**. Bevy's built-in
components/resources (`Camera3d`, `PointLight`, `DirectionalLight`,
`ClearColor`, `GlobalAmbientLight`, `Transform`, …) already are, so most intent
can be checked without touching game code. To assert on your *own* type, add:

```rust
#[derive(Resource, Reflect, Default)]
#[reflect(Resource)]
pub struct Progress { pub lit: u32, pub total: u32 }

// in the plugin:
app.register_type::<Progress>();
```

Keep such reflection additions small and behind normal code (they're cheap and
also help the editor/inspector), or gate them behind `review` if you'd rather
not register them in shipped builds.

## Environment notes (this sandbox)

- No GPU/display: `~/workspace/start_bevy_display.sh` starts Xvfb on `:97`;
  the game uses Mesa software rendering (llvmpipe/lavapipe). The scripts start
  it for you and set `DISPLAY`/`XDG_RUNTIME_DIR`.
- Harmless `wgpu_hal::vulkan` / `drm` / ALSA warnings are expected and ignored;
  only real panics/`ERROR`s fail the smoke layer.
- `curl` + `jq` are used for BRP; the first `--features review` build compiles
  `bevy_remote` and its deps once (several minutes), then is fast.
