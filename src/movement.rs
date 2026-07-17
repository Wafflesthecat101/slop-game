use crate::GameState;
use bevy::prelude::*;

/// A constant 2D velocity, expressed in world units per second.
///
/// This is a small, generic building block: any entity that has both a
/// [`Transform`] and a `Velocity` will be moved by [`MovementPlugin`],
/// independent of *why* it is moving. Today only [`Collectible`]s
/// (see [`crate::game`]) use it, but it is deliberately not gameplay-specific
/// so future entities (projectiles, hazards, particles, ...) can reuse it
/// instead of every feature reinventing its own "add position += velocity *
/// dt" system.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Velocity(pub Vec2);

/// Applies every entity's [`Velocity`] to its [`Transform`] each frame.
///
/// Restricted to [`GameState::Playing`] since that is the only state in
/// which velocity-carrying entities currently exist; this also means the
/// system is skipped entirely (at negligible but non-zero scheduling cost)
/// while on the menu or game-over screen.
pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, apply_velocity.run_if(in_state(GameState::Playing)));
    }
}

fn apply_velocity(time: Res<Time>, mut query: Query<(&Velocity, &mut Transform)>) {
    let dt = time.delta_secs();
    for (velocity, mut transform) in &mut query {
        transform.translation += velocity.0.extend(0.) * dt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn velocity_is_scaled_by_delta_time() {
        // Pure sanity check on the math `apply_velocity` performs, without
        // needing to spin up a full Bevy `App` just to move one entity.
        let velocity = Velocity(Vec2::new(10., -5.));
        let dt = 0.5_f32;
        let delta = velocity.0.extend(0.) * dt;
        assert_eq!(delta, Vec3::new(5., -2.5, 0.));
    }
}
