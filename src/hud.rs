//! Minimal on-screen UI: a centre crosshair, a controls hint, and an
//! objective counter that reflects the [`crate::beacons::Progress`] resource.

use crate::beacons::{Progress, ShrineLit};
use bevy::prelude::*;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud)
            .add_systems(Update, update_objective);
    }
}

/// Marks the objective counter text so `update_objective` can find it.
#[derive(Component)]
struct ObjectiveText;

fn spawn_hud(mut commands: Commands) {
    // Full-screen container that centres the crosshair.
    commands
        .spawn((Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(6.0),
                    height: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
            ));
        });

    commands.spawn((
        Text::new("WASD move  \u{2022}  Mouse look  \u{2022}  Shift run  \u{2022}  Space jump  \u{2022}  Esc free cursor"),
        TextFont {
            font_size: bevy::text::FontSize::Px(16.0),
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));

    // Objective counter, top-left. Populated by `update_objective`.
    commands.spawn((
        ObjectiveText,
        Text::new("Shrines 0"),
        TextFont {
            font_size: bevy::text::FontSize::Px(22.0),
            ..default()
        },
        TextColor(Color::srgb(0.7, 1.0, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(14.0),
            ..default()
        },
    ));
}

/// Keep the objective text in sync with progress, and briefly brighten it
/// (feedback) whenever a shrine is rekindled. Announces completion when every
/// shrine has been lit.
fn update_objective(
    progress: Res<Progress>,
    mut lit: MessageReader<ShrineLit>,
    text: Single<(&mut Text, &mut TextColor), With<ObjectiveText>>,
) {
    let (mut text, mut color) = text.into_inner();

    if progress.is_changed() {
        **text = if progress.lit >= progress.total && progress.total > 0 {
            format!("All {} shrines rekindled!", progress.total)
        } else {
            format!("Shrines {} / {}", progress.lit, progress.total)
        };
    }

    // A rekindle this frame flashes the counter bright yellow; otherwise it
    // eases back to its calm cyan.
    if !lit.is_empty() {
        lit.clear();
        *color = TextColor(Color::srgb(1.0, 1.0, 0.4));
    } else {
        let c = color.0.to_srgba();
        let target = Srgba::rgb(0.7, 1.0, 1.0);
        *color = TextColor(Color::srgb(
            c.red + (target.red - c.red) * 0.08,
            c.green + (target.green - c.green) * 0.08,
            c.blue + (target.blue - c.blue) * 0.08,
        ));
    }
}
