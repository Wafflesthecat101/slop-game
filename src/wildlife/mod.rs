//! Ambient wildlife — cheap, shared-mesh creatures that make the reawakening
//! world feel alive (pillar P3).
//!
//! Each kind of wildlife lives in its own submodule with its own plugin;
//! [`WildlifePlugin`] just composes them. Currently that is [`birds`] (a
//! boids-style flock); future creatures (e.g. deer) will slot in the same way.

use bevy::prelude::*;

pub mod birds;

pub struct WildlifePlugin;

impl Plugin for WildlifePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(birds::BirdsPlugin);
    }
}
