use crate::GameState;
use crate::game::Score;
use crate::ui::{ButtonColors, ChangeState};
use bevy::prelude::*;

/// The screen shown after a round's timer runs out: reports the final
/// score and lets the player either start a fresh round or return to the
/// main menu.
///
/// Button styling/click handling is provided generically by
/// [`crate::ui::UiPlugin`] — this plugin only needs to spawn buttons
/// carrying [`ChangeState`] and [`ButtonColors`], exactly like the main
/// menu does.
pub struct GameOverPlugin;

impl Plugin for GameOverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::GameOver), setup_game_over)
            .add_systems(OnExit(GameState::GameOver), cleanup_game_over);
    }
}

/// Marks every root entity spawned for the game-over screen.
#[derive(Component)]
struct GameOverScreen;

fn setup_game_over(mut commands: Commands, score: Res<Score>) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(16.0),
                ..default()
            },
            GameOverScreen,
        ))
        .with_children(|children| {
            children.spawn((
                Text::new("Time's up!"),
                TextFont {
                    font_size: FontSize::Px(48.0),
                    ..default()
                },
                TextColor(Color::linear_rgb(0.95, 0.95, 0.95)),
            ));
            children.spawn((
                Text::new(format!("Final score: {}", score.0)),
                TextFont {
                    font_size: FontSize::Px(28.0),
                    ..default()
                },
                TextColor(Color::linear_rgb(0.9, 0.9, 0.9)),
            ));

            spawn_menu_button(children, "Play Again", GameState::Playing);
            spawn_menu_button(children, "Main Menu", GameState::Menu);
        });
}

/// Spawns a single labelled button that transitions to `target_state` when
/// pressed. Shared between "Play Again" and "Main Menu" to avoid repeating
/// the same `Node`/`Button`/`ButtonColors` boilerplate twice.
fn spawn_menu_button(children: &mut ChildSpawnerCommands, label: &str, target_state: GameState) {
    let button_colors = ButtonColors::default();
    children
        .spawn((
            Button,
            Node {
                width: Val::Px(180.0),
                height: Val::Px(50.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(button_colors.normal),
            button_colors,
            ChangeState(target_state),
        ))
        .with_child((
            Text::new(label.to_string()),
            TextFont {
                font_size: FontSize::Px(24.0),
                ..default()
            },
            TextColor(Color::linear_rgb(0.9, 0.9, 0.9)),
        ));
}

fn cleanup_game_over(mut commands: Commands, screen: Query<Entity, With<GameOverScreen>>) {
    for entity in &screen {
        commands.entity(entity).despawn();
    }
}
