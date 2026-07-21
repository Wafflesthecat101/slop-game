#![allow(clippy::type_complexity)]

//! A small, efficient 3D open-world walking simulator built on Bevy.
//!
//! The game is composed of small, single-purpose plugins wired together by
//! [`GamePlugin`]:
//!
//! * [`states::StatePlugin`] — the [`states::GameState`] machine
//!   (Boot → MainMenu → Playing → Paused); gameplay only simulates in
//!   `Playing`, and `Esc` toggles pause.
//! * [`world::WorldPlugin`] — procedural terrain, sky/lighting, and scattered
//!   trees and rocks, all built once at startup and textured with the assets
//!   in `assets/textures/`.
//! * [`player::PlayerPlugin`] — a first-person controller (mouse look, WASD,
//!   gravity, jump) with weighty acceleration, head-bob and a sprint FOV kick.
//! * [`beacons::BeaconsPlugin`] — glowing beacon landmarks that double as the
//!   collect-them-all objective (the core gameplay loop).
//! * [`hud::HudPlugin`] — a crosshair, controls hint, and objective counter.
//!
//! The terrain shape lives in [`terrain`] as a pure function shared by the
//! world mesh and the player, so what you see is always what you walk on.

mod beacons;
mod hud;
mod player;
mod states;
mod terrain;
mod world;

use bevy::app::App;
use bevy::prelude::*;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            states::StatePlugin,
            world::WorldPlugin,
            player::PlayerPlugin,
            beacons::BeaconsPlugin,
            hud::HudPlugin,
        ));
    }
}
