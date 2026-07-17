#![allow(clippy::type_complexity)]

mod actions;
mod audio;
mod camera;
mod game;
mod game_over;
mod hud;
mod loading;
mod menu;
mod movement;
mod player;
mod ui;

use crate::actions::ActionsPlugin;
use crate::audio::InternalAudioPlugin;
use crate::camera::CameraPlugin;
use crate::game::GameplayPlugin;
use crate::game_over::GameOverPlugin;
use crate::hud::HudPlugin;
use crate::loading::LoadingPlugin;
use crate::menu::MenuPlugin;
use crate::movement::MovementPlugin;
use crate::player::PlayerPlugin;
use crate::ui::UiPlugin;

use bevy::app::App;
#[cfg(debug_assertions)]
use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};
use bevy::prelude::*;

// This example game uses States to separate logic
// See https://bevy-cheatbook.github.io/programming/states.html
// Or https://github.com/bevyengine/bevy/blob/main/examples/ecs/state.rs
#[derive(States, Default, Clone, Eq, PartialEq, Debug, Hash)]
enum GameState {
    // During the loading State the LoadingPlugin will load our assets
    #[default]
    Loading,
    // During this State the "catch the coins" round is actually played
    Playing,
    // Here the menu is drawn and waiting for player interaction
    Menu,
    // Shown once the round timer runs out: displays the final score and
    // lets the player start a new round or return to the main menu
    GameOver,
}

/// Root plugin wiring together every sub-plugin that makes up the game.
///
/// Each sub-plugin is small and single-purpose (asset loading, menu, input
/// actions, audio, player, generic component-based movement, the scoring/
/// round-timer/collectible gameplay loop, the HUD, the game-over screen,
/// shared UI widgets and the camera) so it can be reasoned about, tested
/// and maintained independently. `GamePlugin` only owns the top-level
/// [`GameState`] machine plus the order-independent composition of those
/// plugins; the relative order in the tuple below does not matter because
/// every system that depends on another's data uses explicit `.after`/
/// `.before` ordering or reads a resource that is guaranteed to already
/// exist (see each sub-plugin's own documentation for details).
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>().add_plugins((
            CameraPlugin,
            UiPlugin,
            LoadingPlugin,
            MenuPlugin,
            ActionsPlugin,
            InternalAudioPlugin,
            PlayerPlugin,
            MovementPlugin,
            GameplayPlugin,
            HudPlugin,
            GameOverPlugin,
        ));

        #[cfg(debug_assertions)]
        {
            app.add_plugins((
                FrameTimeDiagnosticsPlugin::default(),
                LogDiagnosticsPlugin::default(),
            ));
        }
    }
}
