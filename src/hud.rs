//! Minimal on-screen UI: a centre crosshair, a controls hint, and an
//! objective counter that reflects the [`crate::beacons::Score`] resource.

use crate::beacons::{BeaconCollected, Score};
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
        Text::new("Beacons 0"),
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

/// Keep the objective text in sync with the score, and briefly brighten it
/// (feedback) whenever a beacon is collected. Announces victory when all
/// beacons are found.
fn update_objective(
    score: Res<Score>,
    mut collected: MessageReader<BeaconCollected>,
    text: Single<(&mut Text, &mut TextColor), With<ObjectiveText>>,
) {
    let (mut text, mut color) = text.into_inner();

    if score.is_changed() {
        **text = if score.collected >= score.total && score.total > 0 {
            format!("All {} beacons found!", score.total)
        } else {
            format!("Beacons {} / {}", score.collected, score.total)
        };
    }

    // A collection this frame flashes the counter bright yellow; otherwise it
    // eases back to its calm cyan.
    if !collected.is_empty() {
        collected.clear();
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
