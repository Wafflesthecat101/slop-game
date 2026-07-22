//! The title screen shown in [`GameState::MainMenu`]: the game name and
//! Play / Settings / Quit buttons over a calm full-screen panel.
//!
//! The whole UI is one root entity tagged `DespawnOnExit(MainMenu)`, so it
//! spawns on entering the state and is torn down automatically on Play/Quit.
//! Button clicks are handled by the shared systems in [`super`].

use super::{MenuAction, TEXT_COLOR, spawn_button};
use crate::states::GameState;
use crate::world::SKY_COLOR;
use bevy::prelude::*;

pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::MainMenu), spawn_main_menu);
    }
}

fn spawn_main_menu(mut commands: Commands) {
    commands
        .spawn((
            DespawnOnExit(GameState::MainMenu),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(18.0),
                ..default()
            },
            // Opaque panel in the sky colour so the title screen reads as its
            // own calm space rather than the frozen world behind it.
            BackgroundColor(SKY_COLOR),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("Lumen"),
                TextFont {
                    font_size: bevy::text::FontSize::Px(88.0),
                    ..default()
                },
                TextColor(TEXT_COLOR),
                Node {
                    margin: UiRect::bottom(Val::Px(24.0)),
                    ..default()
                },
            ));

            spawn_button(root, "Play", MenuAction::Play, true);
            spawn_button(root, "Settings", MenuAction::Settings, false);
            // Quitting is meaningless on the web (the tab owns the lifecycle),
            // so the button is native-only.
            #[cfg(not(target_arch = "wasm32"))]
            spawn_button(root, "Quit", MenuAction::Quit, true);
        });
}
