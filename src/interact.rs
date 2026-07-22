//! Rekindle interaction — the deliberate "press E to rekindle" prompt.
//!
//! Design pillar P2: lighting a shrine should feel like an *act*, not a thing
//! that happens to you. So instead of [`crate::beacons`] auto-collecting a
//! shrine on proximity, this plugin shows a subtle prompt ("Rekindle — E")
//! whenever the nearest **dormant** shrine is within [`REKINDLE_RADIUS`], and
//! only asks to rekindle it (via [`RekindleRequest`]) when the player presses
//! `E`. It stays gentle (P1: no fail states) — miss the key and nothing
//! happens; the prompt simply hides again when you walk away or once lit.
//!
//! The plugin owns only the *trigger* + the prompt UI; [`crate::beacons`]
//! still owns the *apply* logic (mark lit, bump `Progress`, brighten the orb,
//! emit `ShrineLit`), so the shared Phase-1 contract is unchanged.

use crate::beacons::{REKINDLE_RADIUS, RekindleRequest, Shrine};
use crate::states::GameState;
use bevy::prelude::*;

pub struct InteractPlugin;

impl Plugin for InteractPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_prompt)
            .add_systems(Update, rekindle_prompt.run_if(in_state(GameState::Playing)));
    }
}

/// Marks the "Rekindle — E" prompt text so `rekindle_prompt` can toggle it.
#[derive(Component)]
struct RekindlePrompt;

fn spawn_prompt(mut commands: Commands) {
    commands.spawn((
        RekindlePrompt,
        Text::new("Rekindle \u{2014} E"),
        TextFont {
            font_size: bevy::text::FontSize::Px(20.0),
            ..default()
        },
        TextColor(Color::srgba(1.0, 0.95, 0.8, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Percent(42.0),
            left: Val::Percent(50.0),
            margin: UiRect::left(Val::Px(-60.0)),
            ..default()
        },
        // Hidden until the player is near a dormant shrine.
        Visibility::Hidden,
    ));
}

/// Show the prompt for the nearest dormant shrine in range and, on `E`, ask
/// [`crate::beacons`] to rekindle exactly that shrine. Hides the prompt when no
/// dormant shrine is nearby (falls back gracefully with nothing selected).
fn rekindle_prompt(
    keys: Res<ButtonInput<KeyCode>>,
    player: Single<&Transform, With<Camera3d>>,
    shrines: Query<(Entity, &Transform, &Shrine)>,
    mut prompt: Single<&mut Visibility, With<RekindlePrompt>>,
    mut requests: MessageWriter<RekindleRequest>,
) {
    let p = player.translation;
    let candidates: Vec<(Entity, Vec2, bool)> = shrines
        .iter()
        .map(|(e, tf, s)| (e, Vec2::new(tf.translation.x, tf.translation.z), s.lit))
        .collect();

    let nearest = nearest_dormant_in_range(Vec2::new(p.x, p.z), &candidates, REKINDLE_RADIUS);

    **prompt = if nearest.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };

    if let Some(entity) = nearest
        && keys.just_pressed(KeyCode::KeyE)
    {
        requests.write(RekindleRequest(entity));
    }
}

/// Pure helper: pick the id of the nearest **dormant** shrine whose horizontal
/// distance to `player` is within `radius`, or `None` if there is none.
///
/// Each entry is `(id, horizontal_position, lit)`; `lit` shrines are ignored.
/// Generic over the id so it is testable without constructing an `Entity`.
fn nearest_dormant_in_range<T: Copy>(
    player: Vec2,
    shrines: &[(T, Vec2, bool)],
    radius: f32,
) -> Option<T> {
    shrines
        .iter()
        .filter(|(_, _, lit)| !*lit)
        .map(|(id, pos, _)| (*id, pos.distance(player)))
        .filter(|(_, d)| *d < radius)
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(id, _)| id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_when_all_out_of_range() {
        let shrines = [(0usize, Vec2::new(100.0, 0.0), false)];
        assert_eq!(nearest_dormant_in_range(Vec2::ZERO, &shrines, 4.0), None);
    }

    #[test]
    fn ignores_already_lit_shrines() {
        let shrines = [(0usize, Vec2::new(1.0, 0.0), true)];
        assert_eq!(nearest_dormant_in_range(Vec2::ZERO, &shrines, 4.0), None);
    }

    #[test]
    fn picks_closest_dormant_in_range() {
        let shrines = [
            (0usize, Vec2::new(3.0, 0.0), false),
            (1usize, Vec2::new(1.0, 0.0), false),
            (2usize, Vec2::new(0.5, 0.0), true),
        ];
        assert_eq!(
            nearest_dormant_in_range(Vec2::ZERO, &shrines, 4.0),
            Some(1usize)
        );
    }

    #[test]
    fn radius_is_exclusive_at_the_edge() {
        let shrines = [(0usize, Vec2::new(4.0, 0.0), false)];
        assert_eq!(nearest_dormant_in_range(Vec2::ZERO, &shrines, 4.0), None);
    }

    #[test]
    fn empty_world_is_none() {
        let shrines: [(usize, Vec2, bool); 0] = [];
        assert_eq!(nearest_dormant_in_range(Vec2::ZERO, &shrines, 4.0), None);
    }
}
