# AGENTS.md — repo-specific notes for `slop-game`

## Introduction

**slop-game** is a small, cross-platform 3D game built with the
[Bevy](https://bevyengine.org/) game engine (Rust). The game itself is a
**3D open-world walking simulator**: you spawn into a large procedurally
generated landscape of rolling hills dotted with textured trees and rocks,
and explore it in first person (mouse look + WASD, sprint with Shift, jump
with Space). There is no scoring or timer — it is a calm, freely-explorable
open world.

- **Repository:** `slop-game` — <https://github.com/Wafflesthecat101/slop-game>
- **Local path:** `~/workspace/slop-game` (git remote `origin`, default branch `main`)
- **Engine / language:** Bevy **0.19** on Rust (originally scaffolded from
  [`NiklasEi/bevy_game_template`](https://github.com/NiklasEi/bevy_game_template),
  now heavily rewritten into a 3D game).
- **Cargo package:** `bevy_game` (the crate name still reflects the template;
  the *project* is `slop-game`). The workspace also contains a `mobile/` crate.
- **Targets:** native (Windows/Linux/macOS), Web/WASM (deployed to GitHub
  Pages via `trunk`), and mobile (Android/iOS), all wired up through the
  template's GitHub Actions workflows in `.github/workflows/`.
- **Gameplay code:** the world (terrain mesh, sky/lighting, scattered
  scenery) lives in `src/world.rs`; the first-person player controller (with
  acceleration, head-bob and sprint-FOV game feel) in `src/player.rs`; the
  glowing collectible beacon landmarks and the objective/score loop in
  `src/beacons.rs`; the shared terrain heightfield and biome colouring in
  `src/terrain.rs`; and the crosshair/controls/objective HUD in `src/hud.rs`.
  `GamePlugin` (`src/lib.rs`) just composes those five small plugins.
- **Textures:** the six seamless, tileable ground/object textures in
  `assets/textures/` (`grass`, `dirt`, `rock`, `bark`, `leaves`, `sand`) were
  procedurally generated (512x512 PNGs).
- **`blender_landscape/`** is a separate, standalone asset (a procedurally
  generated Blender landscape: script + `.blend` + rendered PNG) and is *not*
  part of the Bevy build.

This file is the persistent memory for the repo — the sections below capture
the build/test/lint workflow, dependency-upgrade gotchas, how to run the game
headlessly in this sandbox, and the gameplay architecture conventions. Read
it before making changes, and keep it up to date as the project evolves.

## Build/test/lint commands

```
cargo check -p bevy_game
cargo clippy -p bevy_game --all-targets -- -D warnings
cargo fmt            # cargo fmt --check to verify only
cargo test -p bevy_game --lib   # first run takes ~7-8 min (cold registry cache); fast after
cargo check -p bevy_game --target wasm32-unknown-unknown   # matches GH Pages deploy target
cargo check -p mobile
```

### `bevy_lint` (custom Bevy-aware linter)

The binary is `bevy_lint` (not `bevy-lint`). It links against a specific
nightly toolchain's private `librustc_driver*.so`, so it **must** be invoked
through `rustup run <nightly> bevy_lint`, not called directly — and that
nightly needs the `rustc-dev` + `llvm-tools` components installed (that's
what makes the `.so` available on `LD_LIBRARY_PATH`).

The tagged release (`lint-v0.6.0`, pinned to `nightly-2026-01-22`) predates
Rust's stabilization of `cfg_select!`, which `bevy_app` 0.19 now uses
internally, so it fails with `error[E0658]: use of unstable library feature
'cfg_select'` on this project. This was fixed upstream on `bevy_cli`'s
`main` branch by bumping the pinned toolchain to `nightly-2026-04-16`
(https://github.com/TheBevyFlock/bevy_cli/pull/822, closes
https://github.com/TheBevyFlock/bevy_cli/issues/832) — no `main` has been
tagged as a release yet, so we build straight from that commit instead of
patching around the stable tag with unstable-feature flags:

```
# One-time setup: install the toolchain bevy_cli's main branch is pinned to,
# with the components bevy_lint needs to link against.
rustup toolchain install nightly-2026-04-16 --component rustc-dev --component llvm-tools

# Build bevy_lint from that commit (pin by SHA for reproducibility; check
# https://github.com/TheBevyFlock/bevy_cli/commits/main for a newer one, or
# switch to a tagged `lint-vX.Y.Z` release once one supports Bevy 0.19 —
# see the compatibility table below).
rustup run nightly-2026-04-16 cargo install --git https://github.com/TheBevyFlock/bevy_cli.git \
  --rev 8825c5fab8eeaf50a2e3b3d5ed5d1556627d7836 bevy_lint --locked

# Run it — no extra RUSTFLAGS/RUSTC_BOOTSTRAP hacks needed now that the
# toolchain is new enough for `cfg_select!`.
rustup run nightly-2026-04-16 bevy_lint --all-targets
```

This currently exits 0 with no warnings for `bevy_game`. Caveat: as of this
commit, `bevy_cli`'s own compatibility table
(`docs/src/linter/compatibility.md`) still lists this version's supported
Bevy version as 0.18, not 0.19 — the `cfg_select!` compile error is fixed,
but the project hasn't published an official 0.19-verified release yet.
Treat `bevy_lint` results as best-effort until a tagged release lists Bevy
0.19 support, and re-check
https://thebevyflock.github.io/bevy_cli/linter/compatibility.html
periodically to switch to that tag once available (tags are far more stable
than tracking `main`).

`cargo clippy -D warnings` (no special setup needed) remains the
primary/fast lint gate; `bevy_lint` catches Bevy-specific ECS antipatterns
clippy doesn't know about, so re-run it after larger gameplay changes too.

## Dependencies

All direct dependencies in `Cargo.toml`/`mobile/Cargo.toml` track the latest
stable release on crates.io as of this writing (`cargo update` was run after
each bump; `Cargo.lock` reflects it). Notes for future updates:

- The 3D rewrite dropped the arcade game's runtime deps that are no longer
  used: `bevy_kira_audio`, `bevy_asset_loader`, and `webbrowser` (there is no
  audio, no asset-loading state, and no external links). Bevy's feature list
  was also trimmed to the 3D/UI set (no `2d_*`, `scene`, `picking`,
  `sysinfo_plugin`; added `tonemapping_luts`). Re-add any of these only if a
  feature actually needs them.
  - **2026-07-22 (issue #7):** audio infra was **re-added** — this reverses the
    "no audio" note above, but *not* by bringing back `bevy_kira_audio`. Instead
    we re-enabled Bevy's **built-in** audio via its cargo features: added
    `"bevy_audio"` (the `AudioPlugin` + rodio backend, auto-wired into
    `DefaultPlugins` by `default_app`) and the `"wav"` decoder to the `bevy`
    feature list in `Cargo.toml`. Built-in audio was preferred over a new heavy
    crate for minimal footprint. The placeholder smoke-test asset is a tiny
    440 Hz sine committed at `assets/audio/rekindle.wav` (generated with
    Python's stdlib `wave` module since neither `ffmpeg`/`libvorbis` nor `sox`
    is available in this sandbox — hence `wav`, not `vorbis`/`.ogg`). The new
    `src/audio.rs` `AudioPlugin` exposes `play_sfx` (one-shot, despawns) and
    `play_music` (looping), both scaling `Volume::Linear` by the user's
    `Settings` (master × sfx / master × music, clamped); a smoke-test system
    plays `rekindle.wav` on each `ShrineLit` message. **WASM caveat:** Bevy's
    built-in audio compiles and runs on `wasm32-unknown-unknown` (verified with
    `cargo check --target wasm32-unknown-unknown`); it uses the Web Audio
    backend there. The sandbox has no audio device, so native runs still log
    harmless ALSA errors — playback is a no-op but everything still runs.
- `rand` is pinned to the same major version Bevy pulls in transitively
  (via `bevy_math`) so only one copy gets compiled — check
  `cargo tree -i rand` after bumping either `bevy` or `rand` to make sure
  they still agree; a duplicate major version bloats compile times for no
  benefit here. It's used for the deterministic scenery scatter in
  `src/world.rs` (seeded `StdRng`); note the range/bool helpers live on the
  `RngExt` trait in rand 0.10 (`use rand::RngExt;`).
- `winit`, `image`, and `log` are commented "keep in sync with Bevy's
  dependencies" — bump them only to versions Bevy's own `Cargo.toml`
  actually depends on (check `bevy_winit`/`bevy_image`/`bevy_log`'s
  `Cargo.toml` in the registry cache), otherwise Cargo will end up building
  two versions side by side.
- `getrandom` (wasm-only) needs its major version to match whatever `rand`
  requires — `rand` 0.10.x wants `getrandom` 0.4.x, which (unlike 0.3.x)
  auto-detects the wasm backend from the `wasm_js` feature alone, so no
  `.cargo/config.toml` rustflags are needed anymore (removed as part of
  this update; do not re-add them for a modern `getrandom`).
- `embed-resource` (Windows-only build-dependency) went from major version
  1 to 3; the API `build.rs` uses changed from
  `embed_resource::compile(path)` to
  `embed_resource::compile(path, embed_resource::NONE).manifest_optional().unwrap()`.
  `cargo build`/`cargo check` on Linux still fully typecheck this code path
  (only the Windows-only runtime branch is skipped), so you don't need a
  Windows machine to verify a future upgrade compiles.
- `getrandom` 0.3 still shows up in `Cargo.lock` (pulled in transitively by
  `ahash`, itself pulled in by `winit`, on native/non-wasm targets only) —
  that's outside our control since we don't depend on `ahash` directly;
  it's not a duplicate of *our* `getrandom` 0.4 dependency, which only
  applies to the `wasm32` target.

## Running it locally / smoke-testing headlessly

No display server by default; `~/workspace/start_bevy_display.sh` starts an
Xvfb display at `:97`. To run the built binary directly (not through
`bevy_brp_mcp`, which discovers apps via its own `cargo metadata` cwd):

```
export DISPLAY=:97
export XDG_RUNTIME_DIR=/tmp/bevy-xdg-runtime-$(id -u)
mkdir -p "$XDG_RUNTIME_DIR" && chmod 700 "$XDG_RUNTIME_DIR"
cargo build -p bevy_game
ln -sfn ../../assets target/debug/assets   # bevy resolves `assets/` relative to CWD, not the exe
RUST_LOG=warn,bevy_game=info ./target/debug/bevy_game
```

Expect harmless `wgpu_hal::vulkan` / ALSA errors (no GPU/audio device in
this sandbox) — llvmpipe software rendering + no audio device still runs
the game fine at ~60 FPS. No `xdotool`/screenshot tool is installed, so
clicking through UI states can't be automated headlessly here; rely on
unit tests + code review for state-transition logic instead.

## Reviewing a PR at runtime (BRP)

Beyond the static gate, PRs can be verified to actually *run and do what they
intend* using Bevy tooling. See [`docs/REVIEW.md`](docs/REVIEW.md) for the full
process; the essentials for an agent:

- **`review` cargo feature** (`bevy/bevy_remote` + the optional
  `bevy_brp_extras` dep) adds `RemotePlugin` + `RemoteHttpPlugin` and
  `BrpExtrasPlugin` behind `#[cfg(feature = "review")]` in `GamePlugin`, so the
  running game exposes the **Bevy Remote Protocol** on `127.0.0.1:15702`. It is
  compiled out of every shipped build (native + WASM); `cargo check` default
  features stays unchanged. The first `--features review` build compiles
  `bevy_remote` + `bevy_brp_extras` + deps once (~5-6 min), then is fast.
- This bevy_remote (0.19) uses the **`world.*`** JSON-RPC namespace, *not*
  `bevy/*`: `world.query`, `world.list_resources`, `world.get_resources`,
  `world.list_components`, `rpc.discover`. (`bevy/list` returns method-not-found.)
- Query entities by a **fully-qualified reflect path**, e.g. the camera is
  `bevy_camera::components::Camera3d` (not `..::camera::Camera3d`), beacons carry
  `bevy_light::point_light::PointLight`, sun is
  `bevy_light::directional_light::DirectionalLight`; `ClearColor` /
  `GlobalAmbientLight` are readable resources. Built-in Bevy types are already
  reflected/registered; to introspect a *game* type it must
  `#[derive(Reflect)]` + `register_type()` (e.g. gameplay `Score`/`Progress` are
  currently NOT reflected, so BRP reports "Unknown resource type").
- **Scripts** (`gh` is NOT installed — PR fetch uses the GitHub API via `curl`):
  - `scripts/review-pr.sh <pr_number|--branch B|--worktree> [expectations]` —
    full layered review (gate → headless smoke → BRP intent), restores your
    branch, prints a paste-ready summary. **Requires a clean working tree** for
    PR/branch modes (it refuses otherwise, so a dirty tree can't cause a
    false pass on the wrong commit); it verifies HEAD == the fetched SHA. The
    tooling is snapshotted to a temp dir before checkout, so it works on PRs
    that don't contain `scripts/`. If the reviewed code has **no `review`
    feature** (PRs predating this tooling), layers 2–3 are **SKIP**, not FAIL
    → "OVERALL: PASS (gate only)".
  - `scripts/brp-verify.sh [expectations]` — just the headless run + BRP
    assertions; `BRP_BOOT_SECONDS` controls the settle delay before asserting.
  - `scripts/walkthrough.sh [out_dir]` — drive the player over BRP
    (`brp_extras/send_keys`) and capture in-engine viewport screenshots
    (`brp_extras/screenshot`) along a path; renders under the software renderer
    with no external screenshot tool. Screenshot params: `{"path":"…"}`
    (optional `"camera": <entity>`). `send_keys` params:
    `{"keys":["KeyW","ShiftLeft"],"duration_ms":N}` (Bevy `KeyCode` names).
    Also `brp_extras/shutdown` for a clean exit.
  - `scripts/expectations.default.txt` — baseline invariants for `main`
    (Camera3d eq 1, PointLight eq 12, DirectionalLight ge 1, sky/ambient
    resources). Copy + adjust per gameplay PR.

## Architecture

`GamePlugin` (`src/lib.rs`) composes five small, single-purpose plugins.
There is no game-state machine — the whole world is built once at `Startup`
and then simulated every `Update`. Key conventions:

- **The terrain is a single shared pure function.** `terrain::height(x, z)`
  (in `src/terrain.rs`) is a deterministic sum of sines with no allocation.
  Both the mesh builder (`world.rs`) and the player's ground-follow logic
  (`player.rs`) call it, so the visible ground and the surface the player
  walks on can never drift apart. `terrain::normal(x, z)` derives the slope
  from it via finite differences, and `terrain::biome_tint(y)` maps elevation
  to a ground colour (sand → grass → rock). When changing the world's shape,
  edit only `terrain.rs`; everything else follows automatically. These pure
  functions carry `#[cfg(test)]` unit tests (height determinism/amplitude,
  normal points up, biome bands ordered) so logic can be checked without
  spinning up a full `App`/`World`.
- **Scenery is cheap by construction.** `world.rs::scatter_scenery` shares one
  mesh per object kind (trunk, canopy, rock) and a tiny fixed palette of
  materials (bark, rock, three tinted-leaves variants) cloned across ~600
  placements, so the whole forest is a handful of GPU resources. Biome variety
  on the terrain itself is free: per-vertex colours from `biome_tint` multiply
  the one grass texture (no extra draw calls). Placement uses a fixed-seed
  `StdRng` (world is identical every run) and skips steep slopes.
- **Game feel is done without a physics engine.** `player.rs` eases horizontal
  velocity toward the target (acceleration/inertia), adds a distance-driven
  head-bob, and lerps the camera FOV up while sprinting. The head-bob offset is
  re-derived from the terrain height every frame so it never drifts. All player
  state lives on one `Player` component (yaw, pitch, vertical + horizontal
  velocity, bob phase, sprinting) so the systems are self-contained
  `Single<...>` queries. Movement derives its forward/right basis from Bevy's
  `Transform::forward()`/`right()` (flattened to XZ) rather than trig.
- **Atmosphere via built-ins.** The camera (`player.rs`) carries `Hdr` +
  `Bloom` (from `bevy::post_process`) so emissive materials glow, and a
  `DistanceFog` coloured like `world::SKY_COLOR` so the terrain edge dissolves
  into the sky (depth cue + draw-distance mask). The sun is a single low,
  warm `DirectionalLight` with shadows.
- **The gameplay loop is the beacons.** `beacons.rs` places `BEACON_COUNT`
  glowing pillar+orb landmarks on hilltops (deterministic rejection sampling),
  animates the orbs (bob + spin + point light), and auto-collects them on
  player proximity (horizontal distance only). Collection updates the `Score`
  resource and fires a `BeaconCollected` **message** (Bevy 0.19 renamed
  buffered events to *messages*: `#[derive(Message)]`, `add_message`,
  `MessageWriter`/`MessageReader`). The HUD reads `Score` + the message for the
  objective counter and its collect flash.
- **Collision is circle push-out, no physics engine.** Trees, rocks and beacon
  pillars carry a `player::Collider { radius }`; each frame `move_player` (in
  `player.rs`) queries all colliders and, for any whose horizontal circle the
  player overlaps, snaps the player to the circle edge and cancels the
  inward velocity component (so you slide along obstacles). The collider query
  is filtered `Without<Player>` so it stays disjoint from the player's `&mut
  Transform`. Because colliders are plain components, a collected beacon's
  collider disappears when its entity despawns — no separate bookkeeping.
- **Cursor grab/release is split across two systems** (`grab_on_click`,
  `release_cursor`), ordered *before* `mouse_look`/`move_player` via `.chain()`
  so a release takes effect the same frame (no one-frame look lag). Release
  triggers on Escape, window-focus-loss, **or `CursorLeft`**. Winit treats
  `CursorOptions.grab_mode` as our *intent* and does **not** write an external
  unlock back to it (see its `attempt_grab`), so when the web browser exits
  pointer lock on the first Escape itself — swallowing that key event and
  *not* dropping window focus — neither Escape nor focus-loss fires and
  `grab_mode` would stay `Locked`, leaving mouse-look following the now-free
  cursor. Reacting to the `CursorLeft` event the freed cursor emits when it
  leaves the window syncs our state to reality and stops the look. `mouse_look`
  additionally gates on `Window::focused` so an unfocused/background window
  never steers the camera.
- **Cursor grab is a component in Bevy 0.19**, not a `Window` field: query
  `Single<&mut CursorOptions, With<PrimaryWindow>>` to lock/hide it.
- **Ground/object textures are procedurally generated 512x512 seamless PNGs**
  in `assets/textures/`. They tile via UV repeat (see `GROUND_TILING` in
  `world.rs`). Regenerate or add textures there; keep them seamless so the
  terrain has no visible tiling seams. **Tiling requires a `Repeat` sampler:**
  Bevy's default image sampler address mode is `ClampToEdge`, which clamps
  tiled UVs (0..N) to the texture's edge texel and makes the whole surface a
  single flat colour (this was the "ground not rendering" bug). `world.rs`
  loads ground textures via `assets.load_builder().with_settings(repeating_sampler).load(path)`
  (`repeating_sampler` sets `address_mode_*: Repeat`). Object textures (trees,
  rocks) use UVs in `[0,1]` so the sampler mode doesn't matter for them.
  Note `AssetServer::load_with_settings` is deprecated in Bevy 0.19 in favour
  of the `load_builder().with_settings(...).load(...)` form.

## Git workflow — direct pushes to `main` are pre-authorized

The maintainer has granted standing permission for the agent to commit and
push directly to the `main` branch of this repository (`origin` →
`github.com/Wafflesthecat101/slop-game.git`) without opening a pull request or
asking for per-push confirmation.

This authorization applies **only to this repository**.

Guardrails that still apply:
1. Run the project's build/test/lint (see "Build/test/lint commands" above)
   before pushing.
2. Write clear, descriptive commit messages.
3. Never force-push or rewrite already-published history on `main` without
   explicit approval from the maintainer.
4. Server-side branch protection (if any) still governs whether a push is
   accepted; this note does not override GitHub's own rules.
