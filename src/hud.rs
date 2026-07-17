use crate::GameState;
use crate::game::{RoundTimer, Score};
use bevy::prelude::*;

/// Draws the on-screen score and remaining-time readout while the round is
/// in progress.
///
/// Kept separate from [`crate::game`] on purpose: `game.rs` owns the
/// simulation (score, timers, collectibles) and knows nothing about UI,
/// while this plugin only *reads* [`Score`]/[`RoundTimer`] to render text.
/// That separation means the gameplay logic stays fully unit-testable
/// without pulling in any UI/rendering machinery.
pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), setup_hud)
            .add_systems(Update, update_hud.run_if(in_state(GameState::Playing)))
            .add_systems(OnExit(GameState::Playing), cleanup_hud);
    }
}

/// Marks every root entity of the HUD, so [`cleanup_hud`] can despawn it in
/// one query regardless of how many widgets the HUD grows to contain.
#[derive(Component)]
struct Hud;

/// Marks the text entity that displays the current score.
#[derive(Component)]
struct ScoreText;

/// Marks the text entity that displays the remaining round time.
#[derive(Component)]
struct TimerText;

fn setup_hud(mut commands: Commands) {
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::horizontal(Val::Px(16.0)),
            ..default()
        },
        Hud,
        children![
            (
                Text::new("Score: 0"),
                TextFont {
                    font_size: FontSize::Px(28.0),
                    ..default()
                },
                TextColor(Color::linear_rgb(0.95, 0.95, 0.95)),
                ScoreText,
            ),
            (
                Text::new(format_time_remaining(0.0)),
                TextFont {
                    font_size: FontSize::Px(28.0),
                    ..default()
                },
                TextColor(Color::linear_rgb(0.95, 0.95, 0.95)),
                TimerText,
            ),
        ],
    ));
}

fn update_hud(
    score: Res<Score>,
    round_timer: Res<RoundTimer>,
    mut score_text: Query<&mut Text, (With<ScoreText>, Without<TimerText>)>,
    mut timer_text: Query<&mut Text, (With<TimerText>, Without<ScoreText>)>,
) {
    if let Ok(mut text) = score_text.single_mut() {
        text.0 = format!("Score: {}", score.0);
    }
    if let Ok(mut text) = timer_text.single_mut() {
        text.0 = format_time_remaining(round_timer.0.remaining_secs());
    }
}

/// Formats a remaining-seconds value as a whole-second "Time: Ns" string.
///
/// A pure function (no ECS types involved) so the exact text formatting is
/// unit-testable and easy to tweak without touching any system code.
fn format_time_remaining(remaining_secs: f32) -> String {
    format!("Time: {}s", remaining_secs.ceil().max(0.0) as u32)
}

fn cleanup_hud(mut commands: Commands, hud: Query<Entity, With<Hud>>) {
    for entity in &hud {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_whole_seconds_rounded_up() {
        assert_eq!(format_time_remaining(44.2), "Time: 45s");
        assert_eq!(format_time_remaining(0.0), "Time: 0s");
    }

    #[test]
    fn never_shows_negative_time() {
        assert_eq!(format_time_remaining(-1.5), "Time: 0s");
    }
}
