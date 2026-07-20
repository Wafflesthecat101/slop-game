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
  scenery) lives in `src/world.rs`; the first-person player controller in
  `src/player.rs`; the shared terrain heightfield in `src/terrain.rs`; and a
  minimal crosshair/controls HUD in `src/hud.rs`. `GamePlugin` (`src/lib.rs`)
  just composes those four small plugins.
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

## Architecture

`GamePlugin` (`src/lib.rs`) composes four small, single-purpose plugins.
There is no game-state machine — the whole world is built once at `Startup`
and then simulated every `Update`. Key conventions:

- **The terrain is a single shared pure function.** `terrain::height(x, z)`
  (in `src/terrain.rs`) is a deterministic sum of sines with no allocation.
  Both the mesh builder (`world.rs`) and the player's ground-follow logic
  (`player.rs`) call it, so the visible ground and the surface the player
  walks on can never drift apart. `terrain::normal(x, z)` derives the slope
  from it via finite differences. When changing the world's shape, edit only
  `terrain.rs`; everything else follows automatically. These pure functions
  carry `#[cfg(test)]` unit tests (height determinism/amplitude, normal
  points up) so logic can be checked without spinning up a full
  `App`/`World`.
- **Scenery is cheap by construction.** `world.rs::scatter_scenery` shares one
  mesh + one material handle per object kind (trunk, canopy, rock) and clones
  the handles across ~600 placements, so the whole forest is a handful of GPU
  resources. Placement uses a fixed-seed `StdRng` (world is identical every
  run) and skips steep slopes via `terrain::normal(...).y`.
- **All player state lives on one `Player` component** (yaw, pitch, vertical
  velocity) so `move_player`/`mouse_look` are self-contained `Single<...>`
  queries rather than juggling separate resources. Movement derives its
  forward/right basis from Bevy's `Transform::forward()`/`right()` (flattened
  to the XZ plane) rather than recomputing it from trig.
- **Cursor grab is a component in Bevy 0.19**, not a `Window` field: query
  `Single<&mut CursorOptions, With<PrimaryWindow>>` to lock/hide it.
- **Ground/object textures are procedurally generated 512x512 seamless PNGs**
  in `assets/textures/`. They tile via UV repeat (see `GROUND_TILING` in
  `world.rs`). Regenerate or add textures there; keep them seamless so the
  terrain has no visible tiling seams.

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
