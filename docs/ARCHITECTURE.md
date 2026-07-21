# Architecture

This document orients new contributors (humans and agents) to how *slop-game*
/ **Lumen** is wired today. It mirrors the module-ownership rules in
[`CONTRIBUTING.md`](CONTRIBUTING.md) and the crate overview in
[`src/lib.rs`](../src/lib.rs).

## High-level picture

The game is a **Bevy `App`**. `src/main.rs` installs Bevy's `DefaultPlugins`
(window + assets) and then the game crate's root plugin, `GamePlugin`.

`GamePlugin` does **not** contain gameplay systems itself. It only composes
small, single-purpose plugins:

```text
                    main.rs
                       |
                       v
              Bevy DefaultPlugins
                       |
                       v
                  GamePlugin
                       |
       +---------------+---------------+---------------+
       |               |               |               |
       v               v               v               v
  WorldPlugin    PlayerPlugin   BeaconsPlugin     HudPlugin
  (world.rs)     (player.rs)    (beacons.rs)      (hud.rs)
       ^
       |
   terrain.rs  <--- pure height/biome functions
                    (shared by world mesh + player)
```

## Module / plugin map

| Module | Plugin | Responsibility |
|--------|--------|----------------|
| `src/lib.rs` | `GamePlugin` | Crate root. Wires child plugins only. Keep edits here minimal. |
| `src/main.rs` | — | Binary entry: window title, asset root, `GamePlugin`. |
| `src/world.rs` | `WorldPlugin` | Procedural terrain mesh, sky/lighting, scattered trees/rocks, textures. |
| `src/player.rs` | `PlayerPlugin` | First-person controller: mouse look, WASD, jump, sprint FOV, head-bob. |
| `src/beacons.rs` | `BeaconsPlugin` | Glowing beacon landmarks + collect-them-all objective loop. |
| `src/hud.rs` | `HudPlugin` | Crosshair, controls hint, objective counter UI. |
| `src/terrain.rs` | *(no plugin)* | Pure functions for terrain height/biome colouring. Unit-testable without a full `App`. |

### Shared contracts (as they exist today)

These are the main cross-plugin touch points. Prefer extending them carefully
so parallel work does not thrash the same types.

| Contract | Kind | Where | Used for |
|----------|------|-------|----------|
| `BeaconCollected` | `Message` | `beacons.rs` | Emitted when the player collects a beacon; HUD listens to update the objective counter. |
| Beacon progress / remaining state | `Resource` (in beacons module) | `beacons.rs` | Tracks collectible progress for the objective loop. |
| Terrain height / biome helpers | pure functions | `terrain.rs` | Shared by world mesh generation and player ground contact so visuals match collision. |

> Note: as new systems land (day/night, lantern, weather, save, menus), each
> should get **its own module + plugin** rather than growing an existing file.
> That keeps `lib.rs` as a thin wiring layer and reduces merge conflicts.

## Data / message flow (beacon collection)

```text
Player moves near beacon
        |
        v
BeaconsPlugin detects collection
        |
        +--> updates beacon progress Resource
        |
        +--> writes Message: BeaconCollected
                    |
                    v
             HudPlugin reads BeaconCollected
                    |
                    v
             objective counter updates / victory text
```

## Assets and non-game folders

| Path | Role |
|------|------|
| `assets/textures/` | Procedural tileable textures used by the world. |
| `docs/` | Design, contributing process, and this architecture map. |
| `mobile/`, `build/` | Platform packaging (iOS/Android/desktop/web) from the Bevy template. |
| `blender_landscape/` | Standalone Blender landscape asset; not part of the Bevy runtime. |

## Contributor ownership rule (short)

- New gameplay concept → **new file + plugin**.
- Wire it with **one** `mod` line and **one** entry in `GamePlugin`'s
  `add_plugins((...))` tuple.
- Prefer pure helpers (like `terrain`) when logic can be unit-tested without GPU.

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the full parallel-work process and
[`../AGENTS.md`](../AGENTS.md) for build/test/lint gates.
