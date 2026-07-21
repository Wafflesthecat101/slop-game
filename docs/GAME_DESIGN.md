# Lumen — Game Design & Roadmap

> **Working title:** *Lumen* (the project/repo is still `slop-game`).
> This document is the single source of truth for the game's vision, pillars,
> and development roadmap. Every issue in the tracker should trace back to a
> pillar and a roadmap phase below.

## 1. One-line pitch

**A calm, first-person open world that slowly reawakens as you carry light
back into it.** You explore rolling, fog-wrapped hills and rekindle dormant
beacons; each one you light pushes back the dusk, blooms colour and life into
the land, and reveals a little more of a quiet world that remembers you.

## 2. What makes it unique

Most exploration games reward you with combat, loot, or a shrinking map
marker. *Lumen* is built around one distinctive idea:

> **Light is the mechanic, the goal, and the reward — and the world visibly
> responds to how much of it you've restored.**

Three things set it apart:

1. **A world that reawakens.** Progress isn't a number in a menu — it's the
   sky brightening, fog thinning, grass greening, wildlife appearing, and new
   music layering in. The world is a progress bar you *live inside*.
2. **Light as a carried tool.** The player holds a lantern whose glow reveals
   hidden paths, calms or attracts ambient creatures, and matters most at
   night. It's a gentle, non-violent verb in a genre dominated by weapons.
3. **No fail state, no timer, no combat.** It is a *restorative* game. The
   only pressure is curiosity. This is a deliberate, defensible identity — we
   protect it in every design decision (see Pillars).

## 3. Design pillars

These are the non-negotiables. When a feature is proposed, it must strengthen
(or at least not violate) all four.

- **P1 — Calm, never stressful.** No combat, no fail states, no punishing
  timers. Tension comes only from curiosity and atmosphere.
- **P2 — The world reacts.** Player progress is expressed *in the world*
  (light, colour, weather, life, sound) before it is ever shown in UI.
- **P3 — Legible from afar.** You can always see somewhere to go. Beacons,
  vistas, and light guide the eye. No hand-holding minimaps.
- **P4 — Cheap by construction.** Procedural, shared meshes/materials,
  pure-function terrain. It must run at 60 FPS in a browser (WASM) on a
  laptop. Beauty comes from light and composition, not polygon count.

## 4. Core loop

```
Spot a dormant beacon on the horizon  ──▶  Traverse to it (hills, water,
        ▲                                    hidden paths revealed by lantern)
        │                                              │
        │                                              ▼
World reawakens a little more  ◀──  Rekindle it (light returns, world responds)
(sky, fog, colour, life, music)
```

Secondary loops: discover **points of interest** (ruins, groves, overlooks)
that hold optional narrative fragments and cosmetic lantern upgrades; use
**photo mode** to capture the reawakened world.

## 5. Current state (baseline)

The repository already implements a strong foundation for this vision:

- **Terrain** — a pure `terrain::height(x, z)` sum-of-sines heightfield shared
  by the mesh and the player, with elevation-based biome tinting.
- **Player** — first-person controller with acceleration/inertia, head-bob,
  sprint-FOV, gravity/jump, and circle push-out collision.
- **Beacons** — 12 glowing collectible pillars on hilltops (the proto version
  of "rekindling"), with a score/objective loop.
- **Atmosphere** — HDR + bloom, distance fog matched to the sky, a warm low
  sun.
- **HUD** — crosshair, controls hint, objective counter.

The roadmap below evolves this baseline into *Lumen* without throwing any of
it away. Beacons become **shrines to rekindle**; the score loop becomes the
**world-reawakening** system.

## 6. Roadmap (phases = GitHub milestones)

Each phase is a milestone. Issues are scoped so that agents can work in
**parallel with minimal merge conflicts** — see `CONTRIBUTING.md` §"Working in
a large team".

### Phase 0 — Foundations & Contributor Infrastructure
*Goal: unblock a large parallel workforce.*
Game-state machine (Boot → Menu → Playing → Paused), main menu, pause menu,
a settings resource, save/load of world progress, audio playback
infrastructure, and CI/lint hardening. Nothing here changes the core fantasy;
it builds the scaffolding every later feature needs.

### Phase 1 — Core Identity: "Bring Back the Light"
*Goal: make the unique hook real.*
Turn beacon collection into **rekindling**; add a **day/night cycle** driven
by progress; a **world-reawakening** system (fog/colour/light respond to how
many shrines are lit); and the **carried lantern** (light you hold, toggle,
and that reveals things).

### Phase 2 — World & Traversal
*Goal: a world worth exploring.*
Richer terrain (layered noise, rivers/lakes/water plane), distinct **regions/
biomes** with their own palettes and scenery, **points of interest**, and new
traversal verbs (climbing, gliding, wind currents).

### Phase 3 — Life & Atmosphere
*Goal: make the world feel alive.*
Ambient **wildlife/creatures** (flocking birds, wandering deer, fireflies),
a **weather system** (clouds, rain, wind), **generative/layered ambient
audio** tied to progress and biome, and particle polish.

### Phase 4 — Player Experience & Accessibility
*Goal: feel great to play for everyone.*
**Photo mode**, HUD/UX polish, a full **settings menu** (sensitivity, FOV,
volume, quality), **gamepad support**, and **accessibility** options
(colourblind-safe palettes, motion/head-bob toggle, subtitles for audio cues).

### Phase 5 — Content, Narrative & Polish
*Goal: depth and shine.*
Optional **narrative fragments** / lore collectibles, gentle **environmental
puzzles** to reach some shrines, **LOD & performance** work, and **WASM build
optimisation** for the GitHub Pages deploy.

## 7. Anti-goals (things we will NOT build)

- Combat, weapons, health bars, or enemies that can hurt you.
- A minimap or quest markers that remove the joy of noticing.
- Microtransactions, timers, or "engagement" dark patterns.
- Heavyweight physics or asset pipelines that break the 60-FPS-in-browser
  budget.

## 8. Success criteria

- Loads and runs at ~60 FPS in a desktop browser via the existing GitHub
  Pages WASM deploy.
- A first-time player, with no tutorial text, understands within 30 seconds
  that they should walk toward a glowing beacon — and *feels* the world change
  when they light one.
- The build stays green on `cargo test`/`clippy`/`fmt` across Windows, Linux,
  and macOS.
