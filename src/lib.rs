#![allow(clippy::type_complexity)]

//! A small, efficient 3D open-world walking simulator built on Bevy.
//!
//! The game is composed of three tiny, single-purpose plugins wired together
//! by [`GamePlugin`]:
//!
//! * [`world::WorldPlugin`] — procedural terrain, sky/lighting, and scattered
//!   trees and rocks, all built once at startup and textured with the assets
//!   in `assets/textures/`.
//! * [`player::PlayerPlugin`] — a first-person controller (mouse look, WASD,
//!   gravity, jump) that walks on the terrain surface.
//! * [`hud::HudPlugin`] — a crosshair and a controls hint.
//!
//! The terrain shape lives in [`terrain`] as a pure function shared by the
//! world mesh and the player, so what you see is always what you walk on.

mod hud;
mod player;
mod terrain;
mod world;

use bevy::app::App;
use bevy::prelude::*;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((world::WorldPlugin, player::PlayerPlugin, hud::HudPlugin));
    }
}
