//! The game-state machine — the scaffolding every menu, pause, photo-mode and
//! save feature builds on.
//!
//! The world is still built once at `Startup` (cheap, and pause-safe because
//! the spawned entities simply sit still while paused); what the state machine
//! gates is the *simulation*. Gameplay `Update` systems in [`crate::player`]
//! and [`crate::beacons`] run only in [`GameState::Playing`] via
//! `run_if(in_state(...))`, so entering [`GameState::Paused`] freezes movement,
//! rekindling and orb animation without tearing anything down.
//!
//! Flow: start in [`GameState::Boot`], then advance to `Playing`. Until the
//! main-menu issue (#3) lands there is no menu UI, so `Boot` transitions
//! straight to `Playing`; #3 will insert `MainMenu` in between and add a Play
//! button that drives `MainMenu → Playing`. `Esc` toggles `Playing ↔ Paused`.
//!
//! Cursor grab is owned here for state transitions: entering `Playing` locks
//! and hides the cursor; entering `Paused` (or `MainMenu`) frees it. This
//! coordinates with the reactive cursor-release logic in [`crate::player`]
//! (which still handles focus-loss and the web pointer-lock quirk while
//! playing).

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

/// High-level game flow. Gameplay simulation only runs in [`Self::Playing`].
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameState {
    /// One-frame entry state; immediately advances (to `MainMenu` once that
    /// exists, to `Playing` for now).
    #[default]
    Boot,
    /// Title screen (owned by the main-menu issue #3). No UI yet.
    MainMenu,
    /// Active gameplay: the world simulates and the player is in control.
    Playing,
    /// Gameplay frozen; overlay owned by the pause-menu issue #4.
    Paused,
}

pub struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .add_systems(Update, boot_to_playing.run_if(in_state(GameState::Boot)))
            .add_systems(
                Update,
                toggle_pause
                    .run_if(in_state(GameState::Playing).or_else(in_state(GameState::Paused))),
            )
            .add_systems(OnEnter(GameState::Playing), grab_cursor)
            .add_systems(OnEnter(GameState::Paused), release_cursor)
            .add_systems(OnEnter(GameState::MainMenu), release_cursor);
    }
}

/// Placeholder boot step: with no main menu yet, drop straight into gameplay.
/// The main-menu issue (#3) will retarget this to `MainMenu`.
fn boot_to_playing(mut next: ResMut<NextState<GameState>>) {
    next.set(GameState::Playing);
}

/// `Esc` toggles between playing and paused. In `Playing` it opens the pause
/// state (freezing the sim); in `Paused` it resumes. The `OnEnter` cursor
/// systems handle grab/release on each side of the transition.
fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    next.set(match state.get() {
        GameState::Playing => GameState::Paused,
        _ => GameState::Playing,
    });
}

fn grab_cursor(mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>) {
    cursor.visible = false;
    cursor.grab_mode = CursorGrabMode::Locked;
}

fn release_cursor(mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>) {
    cursor.visible = true;
    cursor.grab_mode = CursorGrabMode::None;
}
