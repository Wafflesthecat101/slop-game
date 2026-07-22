//! The pause overlay shown in [`GameState::Paused`]: a translucent scrim over
//! the frozen world with Resume / Settings / Main Menu buttons.
//!
//! Gameplay is already frozen by the state machine (player/beacon systems are
//! gated on `Playing`), so this module only draws the overlay and lets the
//! shared button handler in [`super`] switch state. The overlay is one root
//! entity tagged `DespawnOnExit(Paused)`, so it despawns the moment we leave
//! `Paused` (Resume or Main Menu). The cursor is released/re-grabbed by
//! [`crate::states`] on the state transitions, not here.

use super::{MenuAction, TEXT_COLOR, spawn_button};
use crate::states::GameState;
use bevy::prelude::*;

pub struct PausePlugin;

impl Plugin for PausePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Paused), spawn_pause_menu);
    }
}

fn spawn_pause_menu(mut commands: Commands) {
    commands
        .spawn((
            DespawnOnExit(GameState::Paused),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(18.0),
                ..default()
            },
            // Translucent dark scrim: the paused world stays visible behind it.
            BackgroundColor(Color::srgba(0.03, 0.05, 0.09, 0.75)),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("Paused"),
                TextFont {
                    font_size: bevy::text::FontSize::Px(64.0),
                    ..default()
                },
                TextColor(TEXT_COLOR),
                Node {
                    margin: UiRect::bottom(Val::Px(24.0)),
                    ..default()
                },
            ));

            spawn_button(root, "Resume", MenuAction::Resume, true);
            spawn_button(root, "Settings", MenuAction::Settings, false);
            spawn_button(root, "Main Menu", MenuAction::ToMainMenu, true);
        });
}
