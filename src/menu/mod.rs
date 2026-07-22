//! Menu UI: the [`main_menu`] title screen and the [`pause`] overlay, composed
//! into one [`MenuPlugin`].
//!
//! Both menus are built from the same calm palette and share a single button
//! look-and-feel and a single click handler, so this module owns the pieces
//! they have in common (the [`MenuAction`] a button carries, the shared button
//! builder, and the interaction systems) while each sub-module owns only its
//! own layout and which state it is scoped to.
//!
//! State scoping is handled with Bevy's [`DespawnOnExit`] component
//! (`OnEnter` spawns the UI, leaving the state despawns it) so nothing leaks
//! between screens. The cursor is *not* touched here: [`crate::states`] already
//! grabs the cursor on entering `Playing` and releases it on entering
//! `MainMenu`/`Paused`, which is exactly the behaviour the menus need — so a
//! Resume/Play button just switches state and the state machine re-grabs.

mod main_menu;
mod pause;

use crate::states::GameState;
use bevy::prelude::*;

/// Calm palette, keyed off `world::SKY_COLOR` (a soft blue). Buttons are a
/// muted slate that brightens toward the sky on hover/press; the placeholder
/// Settings button stays dim to read as unavailable.
const BUTTON_NORMAL: Color = Color::srgb(0.22, 0.28, 0.38);
const BUTTON_HOVER: Color = Color::srgb(0.32, 0.41, 0.55);
const BUTTON_PRESSED: Color = Color::srgb(0.44, 0.55, 0.72);
const BUTTON_DISABLED: Color = Color::srgb(0.18, 0.21, 0.26);
const TEXT_COLOR: Color = Color::srgb(0.93, 0.96, 1.0);
const TEXT_DISABLED: Color = Color::srgb(0.50, 0.55, 0.62);

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((main_menu::MainMenuPlugin, pause::PausePlugin))
            .add_systems(
                Update,
                (button_visuals, button_actions)
                    .run_if(in_state(GameState::MainMenu).or_else(in_state(GameState::Paused))),
            );
    }
}

/// What a menu button does when clicked. Shared by both menus so a single
/// handler drives every button.
#[derive(Component, Clone, Copy)]
enum MenuAction {
    /// Main menu → start the game.
    Play,
    /// Pause menu → return to gameplay.
    Resume,
    /// Pause menu → back out to the title screen.
    ToMainMenu,
    /// Placeholder: the settings screen is a separate future issue, so this
    /// button is spawned disabled and does nothing when interacted with.
    Settings,
    /// Quit the game (native only; not spawned on WASM).
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Quit,
}

/// Marks a button as non-interactive: it renders dimmed and both the visual
/// and action systems skip it.
#[derive(Component)]
struct Disabled;

/// Recolour buttons on hover/press for feedback. Disabled buttons are skipped
/// so they stay visibly inert.
fn button_visuals(
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>, Without<Disabled>),
    >,
) {
    for (interaction, mut color) in &mut buttons {
        *color = match interaction {
            Interaction::Pressed => BUTTON_PRESSED,
            Interaction::Hovered => BUTTON_HOVER,
            Interaction::None => BUTTON_NORMAL,
        }
        .into();
    }
}

/// Run a clicked button's [`MenuAction`]. Disabled buttons are filtered out,
/// so the placeholder Settings button is a no-op by construction.
fn button_actions(
    buttons: Query<
        (&Interaction, &MenuAction),
        (Changed<Interaction>, With<Button>, Without<Disabled>),
    >,
    mut next: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            MenuAction::Play | MenuAction::Resume => next.set(GameState::Playing),
            MenuAction::ToMainMenu => next.set(GameState::MainMenu),
            MenuAction::Settings => {} // placeholder — see MenuAction::Settings
            MenuAction::Quit => {
                exit.write(AppExit::Success);
            }
        }
    }
}

/// Spawn a styled menu button as a child of `parent`. When `enabled` is false
/// the button is dimmed and tagged [`Disabled`] so the shared systems ignore
/// it (used for the placeholder Settings entry).
fn spawn_button(parent: &mut ChildSpawnerCommands, label: &str, action: MenuAction, enabled: bool) {
    let (bg, fg) = if enabled {
        (BUTTON_NORMAL, TEXT_COLOR)
    } else {
        (BUTTON_DISABLED, TEXT_DISABLED)
    };
    let mut button = parent.spawn((
        Button,
        action,
        Node {
            width: Val::Px(240.0),
            height: Val::Px(56.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(bg),
    ));
    if !enabled {
        button.insert(Disabled);
    }
    button.with_children(|b| {
        b.spawn((
            Text::new(label),
            TextFont {
                font_size: bevy::text::FontSize::Px(24.0),
                ..default()
            },
            TextColor(fg),
        ));
    });
}
