use crate::GameState;
use bevy::prelude::*;

/// Generic, reusable UI building blocks shared by every screen that shows
/// clickable buttons (currently [`crate::menu`] and [`crate::game_over`]).
///
/// Before this module existed, the main menu implemented its own
/// button-coloring and click-handling logic directly in `menu.rs`. Once the
/// game-over screen needed "Play Again" / "Main Menu" buttons with the same
/// hover/press behavior, duplicating that logic would have meant two
/// separate systems to keep in sync. Factoring it out here means any screen
/// can build a button by attaching [`ButtonColors`] plus either
/// [`ChangeState`] or [`OpenLink`], and [`UiPlugin`]'s single system handles
/// the rest.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_button_interactions);
    }
}

/// The background color a button should use while idle vs. while hovered.
#[derive(Component)]
pub struct ButtonColors {
    pub normal: Color,
    pub hovered: Color,
}

impl Default for ButtonColors {
    fn default() -> Self {
        ButtonColors {
            normal: Color::linear_rgb(0.15, 0.15, 0.15),
            hovered: Color::linear_rgb(0.25, 0.25, 0.25),
        }
    }
}

/// Attach to a button to make clicking it request a transition to `.0`.
#[derive(Component)]
pub struct ChangeState(pub GameState);

/// Attach to a button to make clicking it open `.0` in the system browser.
#[derive(Component)]
pub struct OpenLink(pub &'static str);

/// Drives hover/press feedback for every button in the UI tree, regardless
/// of which screen it belongs to.
///
/// Runs unconditionally (in every [`GameState`]) since it is filtered to
/// `Changed<Interaction>` and there are at most a handful of buttons on
/// screen at once, so the cost of leaving it always-on is negligible and it
/// avoids every screen having to remember to gate it behind its own state.
fn handle_button_interactions(
    mut next_state: ResMut<NextState<GameState>>,
    mut interaction_query: Query<
        (
            &Interaction,
            &mut BackgroundColor,
            &ButtonColors,
            Option<&ChangeState>,
            Option<&OpenLink>,
        ),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, mut color, button_colors, change_state, open_link) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                if let Some(state) = change_state {
                    next_state.set(state.0.clone());
                } else if let Some(link) = open_link
                    && let Err(error) = webbrowser::open(link.0)
                {
                    warn!("Failed to open link {error:?}");
                }
            }
            Interaction::Hovered => {
                *color = button_colors.hovered.into();
            }
            Interaction::None => {
                *color = button_colors.normal.into();
            }
        }
    }
}
