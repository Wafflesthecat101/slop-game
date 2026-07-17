# AGENTS.md — repo-specific notes for `slop-game`

Bevy 0.19 game, scaffolded from `bevy_game_template`. Package name is
`bevy_game` (see `Cargo.toml`); workspace also contains `mobile/`.

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

The binary is `bevy_lint` (not `bevy-lint`). It requires the exact nightly
toolchain pinned in `bevy_cli`'s `rust-toolchain.toml`
(`~/.cargo/git/checkouts/bevy_cli-*/*/rust-toolchain.toml`, currently
`nightly-2026-01-22`) because it links against that toolchain's private
`librustc_driver*.so` — calling the raw `~/.cargo/bin/bevy_lint` binary
under the default `stable` toolchain fails with
`error while loading shared libraries: librustc_driver-*.so: cannot open
shared object file`. Fix: invoke it through `rustup run <that nightly>
bevy_lint`, which puts the matching driver `.so` on `LD_LIBRARY_PATH`
automatically.

That still isn't enough on its own here: this pinned nightly (~Jan 2026)
predates stabilization of the `cfg_select!` macro that `bevy_app` 0.19 now
uses internally, so every run fails with
`error[E0658]: use of unstable library feature 'cfg_select'` inside
`bevy_app`'s vendored source (not our code) unless the feature is enabled.
Workaround — inject the crate attribute into every crate being built via
`RUSTFLAGS`, and allow unstable flags on the pinned nightly via
`RUSTC_BOOTSTRAP=1`:

```
RUSTC_BOOTSTRAP=1 RUSTFLAGS="-Zcrate-attr=feature(cfg_select)" \
  rustup run nightly-2026-01-22 bevy_lint --all-targets
```

This currently exits 0 with no warnings for `bevy_game`. `cargo clippy -D
warnings` (no special setup needed) remains the primary/fast lint gate;
`bevy_lint` catches Bevy-specific ECS antipatterns clippy doesn't know
about, so re-run it after larger gameplay changes too.

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

`GamePlugin` (src/lib.rs) composes small single-purpose plugins gated by a
`GameState` enum: `Loading -> Menu <-> Playing <-> GameOver`. Key
conventions established when the gameplay loop was added:

- Any entity/resource created `OnEnter(Playing)` **must** be cleaned up
  `OnExit(Playing)` — the state can now be re-entered (Play Again / Main
  Menu), so anything spawned once per "game start" assumption from the
  original one-shot template needs an explicit teardown. Bugs already
  found and fixed this way: duplicate camera (moved to `camera.rs`
  `Startup`), duplicate player sprites (`player.rs` `despawn_player`),
  leaked looping audio instance (`audio.rs` `stop_audio`).
- Gameplay math (difficulty curve, collision radius check, time
  formatting) is factored into plain functions with `#[cfg(test)]` unit
  tests, decoupled from ECS `Query`/`Res` plumbing — keeps tests fast and
  avoids needing a full `App`/`World` for pure logic.
- Shared button widgets (`ButtonColors`, `ChangeState`, `OpenLink`) live in
  `src/ui.rs` and are used by both `menu.rs` and `game_over.rs` — add new
  screens' buttons there rather than re-implementing hover/click handling.
