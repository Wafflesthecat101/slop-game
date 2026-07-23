//! The settings screen: a calm full-screen panel for editing the [`Settings`]
//! resource live (mouse sensitivity, FOV, master/music/SFX volume, quality).
//!
//! It is reachable from both the title screen and the pause overlay via their
//! shared `Settings` button (wired in [`super`]). Rather than adding a new
//! [`GameState`] variant (owned by [`crate::states`]), this screen is its own
//! tiny, independent [`SettingsMenu`] state machine layered *on top of*
//! whichever menu opened it: the underlying `GameState` (`MainMenu` or
//! `Paused`) is left untouched, so pressing **Back** simply closes this overlay
//! and reveals the menu beneath it. The panel is opaque (the sky colour), so it
//! fully hides that menu while open, and its default `FocusPolicy::Block` stops
//! clicks from reaching the buttons behind it.
//!
//! Editing is a calm `-`/`+` stepper per value (Bevy has no built-in slider);
//! each press mutates [`Settings`], which the player controller and audio read
//! from directly and which [`crate::save`] autosaves on change — so edits are
//! live *and* persisted with no extra plumbing here. The step/clamp/format
//! logic is factored into pure helpers so it can be unit-tested without a Bevy
//! `App` (see the `#[cfg(test)]` module).

use super::{BUTTON_NORMAL, TEXT_COLOR};
use crate::settings::{Quality, Settings};
use crate::states::GameState;
use crate::world::SKY_COLOR;
use bevy::prelude::*;

/// Overlay visibility as its own state, independent of [`GameState`] so the
/// screen can sit over either the main menu or the pause menu.
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum SettingsMenu {
    /// Not shown (the default): the underlying menu is in control.
    #[default]
    Closed,
    /// The settings panel is open on top of the current menu.
    Open,
}

pub(super) struct SettingsMenuPlugin;

impl Plugin for SettingsMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<SettingsMenu>()
            .add_systems(OnEnter(SettingsMenu::Open), spawn_settings)
            .add_systems(
                Update,
                (apply_stepper, refresh_labels, back_button).run_if(in_state(SettingsMenu::Open)),
            )
            // If the underlying menu leaves (e.g. Esc resumes gameplay while the
            // panel is open), make sure the overlay doesn't linger.
            .add_systems(OnExit(GameState::MainMenu), close_settings)
            .add_systems(OnExit(GameState::Paused), close_settings);
    }
}

/// The editable values, used to tag both a stepper button and the text that
/// displays that value's current setting.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Sensitivity,
    Fov,
    Master,
    Music,
    Sfx,
    Quality,
}

/// A `-`/`+` button: which [`Field`] it edits and in which direction.
#[derive(Component, Clone, Copy)]
struct Stepper {
    field: Field,
    dir: i32,
}

/// The text node showing a [`Field`]'s current value, refreshed on change.
#[derive(Component, Clone, Copy)]
struct ValueLabel(Field);

/// The button that closes the panel.
#[derive(Component)]
struct BackButton;

// Edit ranges and step sizes. Chosen to stay in comfortable, calm bounds
// around the defaults in [`Settings`].
const SENS_MIN: f32 = 0.0005;
const SENS_MAX: f32 = 0.0100;
const SENS_STEP: f32 = 0.0005;
const FOV_MIN: f32 = 0.70; // ~40°
const FOV_MAX: f32 = 1.75; // ~100°
const FOV_STEP: f32 = 0.05;
const VOL_MIN: f32 = 0.0;
const VOL_MAX: f32 = 1.0;
const VOL_STEP: f32 = 0.05;

/// Move `value` by `dir` steps of `step`, clamped to `[min, max]` and rounded
/// to four decimals so repeated float additions don't drift.
fn stepped(value: f32, dir: i32, step: f32, min: f32, max: f32) -> f32 {
    let raw = (value + dir as f32 * step).clamp(min, max);
    (raw * 10_000.0).round() / 10_000.0
}

/// Step the quality preset up/down its `Low → Medium → High` order, clamping at
/// the ends (no wrap-around, which would feel jumpy).
fn cycle_quality(q: Quality, dir: i32) -> Quality {
    let idx = match q {
        Quality::Low => 0i32,
        Quality::Medium => 1,
        Quality::High => 2,
    };
    match (idx + dir).clamp(0, 2) {
        0 => Quality::Low,
        1 => Quality::Medium,
        _ => Quality::High,
    }
}

fn quality_label(q: Quality) -> &'static str {
    match q {
        Quality::Low => "Low",
        Quality::Medium => "Medium",
        Quality::High => "High",
    }
}

/// Apply a single stepper press to `settings`. Pure so the clamp/step logic is
/// unit-testable. Stepping the base FOV shifts the sprint FOV by the same
/// amount, preserving the "sense of speed" gap between them.
fn apply_step(settings: &mut Settings, field: Field, dir: i32) {
    match field {
        Field::Sensitivity => {
            settings.mouse_sensitivity = stepped(
                settings.mouse_sensitivity,
                dir,
                SENS_STEP,
                SENS_MIN,
                SENS_MAX,
            );
        }
        Field::Fov => {
            let new = stepped(settings.fov, dir, FOV_STEP, FOV_MIN, FOV_MAX);
            let delta = new - settings.fov;
            settings.fov = new;
            settings.fov_run += delta;
        }
        Field::Master => {
            settings.master_volume =
                stepped(settings.master_volume, dir, VOL_STEP, VOL_MIN, VOL_MAX);
        }
        Field::Music => {
            settings.music_volume = stepped(settings.music_volume, dir, VOL_STEP, VOL_MIN, VOL_MAX);
        }
        Field::Sfx => {
            settings.sfx_volume = stepped(settings.sfx_volume, dir, VOL_STEP, VOL_MIN, VOL_MAX);
        }
        Field::Quality => settings.quality = cycle_quality(settings.quality, dir),
    }
}

/// The human-readable current value for a field (percent for volumes, degrees
/// for FOV, the raw radians-per-pixel for sensitivity, the preset for quality).
fn value_string(settings: &Settings, field: Field) -> String {
    match field {
        Field::Sensitivity => format!("{:.4}", settings.mouse_sensitivity),
        Field::Fov => format!("{:.0}°", settings.fov.to_degrees()),
        Field::Master => format!("{:.0}%", settings.master_volume * 100.0),
        Field::Music => format!("{:.0}%", settings.music_volume * 100.0),
        Field::Sfx => format!("{:.0}%", settings.sfx_volume * 100.0),
        Field::Quality => quality_label(settings.quality).to_string(),
    }
}

fn text_font(px: f32) -> TextFont {
    TextFont {
        font_size: bevy::text::FontSize::Px(px),
        ..default()
    }
}

fn spawn_settings(mut commands: Commands, settings: Res<Settings>) {
    commands
        .spawn((
            DespawnOnExit(SettingsMenu::Open),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(14.0),
                ..default()
            },
            // Opaque sky-coloured panel: fully hides the menu underneath and
            // reads as its own calm space.
            BackgroundColor(SKY_COLOR),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("Settings"),
                text_font(64.0),
                TextColor(TEXT_COLOR),
                Node {
                    margin: UiRect::bottom(Val::Px(24.0)),
                    ..default()
                },
            ));

            spawn_row(root, &settings, Field::Sensitivity, "Mouse Sensitivity");
            spawn_row(root, &settings, Field::Fov, "Field of View");
            spawn_row(root, &settings, Field::Master, "Master Volume");
            spawn_row(root, &settings, Field::Music, "Music Volume");
            spawn_row(root, &settings, Field::Sfx, "SFX Volume");
            spawn_row(root, &settings, Field::Quality, "Quality");

            root.spawn((
                Button,
                BackButton,
                Node {
                    width: Val::Px(240.0),
                    height: Val::Px(56.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    margin: UiRect::top(Val::Px(24.0)),
                    ..default()
                },
                BackgroundColor(BUTTON_NORMAL),
            ))
            .with_children(|b| {
                b.spawn((Text::new("Back"), text_font(24.0), TextColor(TEXT_COLOR)));
            });
        });
}

/// One `label   [-] value [+]` row for a single [`Field`].
fn spawn_row(root: &mut ChildSpawnerCommands, settings: &Settings, field: Field, name: &str) {
    root.spawn(Node {
        width: Val::Px(520.0),
        height: Val::Px(48.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::SpaceBetween,
        ..default()
    })
    .with_children(|row| {
        row.spawn((
            Text::new(name),
            text_font(22.0),
            TextColor(TEXT_COLOR),
            Node {
                width: Val::Px(260.0),
                ..default()
            },
        ));
        row.spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(14.0),
            ..default()
        })
        .with_children(|cluster| {
            spawn_step_button(cluster, "-", Stepper { field, dir: -1 });
            cluster.spawn((
                Text::new(value_string(settings, field)),
                text_font(22.0),
                TextColor(TEXT_COLOR),
                ValueLabel(field),
                Node {
                    width: Val::Px(90.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
            ));
            spawn_step_button(cluster, "+", Stepper { field, dir: 1 });
        });
    });
}

fn spawn_step_button(parent: &mut ChildSpawnerCommands, label: &str, stepper: Stepper) {
    parent
        .spawn((
            Button,
            stepper,
            Node {
                width: Val::Px(44.0),
                height: Val::Px(44.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(BUTTON_NORMAL),
        ))
        .with_children(|b| {
            b.spawn((Text::new(label), text_font(24.0), TextColor(TEXT_COLOR)));
        });
}

/// Apply stepper clicks to [`Settings`]. Only writes when the value actually
/// changes, so clamped-at-the-edge clicks don't churn change detection (and
/// thus don't trigger a needless autosave).
fn apply_stepper(
    mut settings: ResMut<Settings>,
    steppers: Query<(&Interaction, &Stepper), (Changed<Interaction>, With<Button>)>,
) {
    for (interaction, stepper) in &steppers {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let mut candidate = *settings;
        apply_step(&mut candidate, stepper.field, stepper.dir);
        if candidate != *settings {
            *settings = candidate;
        }
    }
}

/// Refresh the on-screen values whenever [`Settings`] changes.
fn refresh_labels(settings: Res<Settings>, mut labels: Query<(&ValueLabel, &mut Text)>) {
    if !settings.is_changed() {
        return;
    }
    for (label, mut text) in &mut labels {
        text.0 = value_string(&settings, label.0);
    }
}

fn back_button(
    back: Query<&Interaction, (Changed<Interaction>, With<Button>, With<BackButton>)>,
    mut next: ResMut<NextState<SettingsMenu>>,
) {
    for interaction in &back {
        if *interaction == Interaction::Pressed {
            next.set(SettingsMenu::Closed);
        }
    }
}

fn close_settings(mut next: ResMut<NextState<SettingsMenu>>) {
    next.set(SettingsMenu::Closed);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn stepped_clamps_to_bounds() {
        assert!(approx(
            stepped(SENS_MAX, 1, SENS_STEP, SENS_MIN, SENS_MAX),
            SENS_MAX
        ));
        assert!(approx(
            stepped(SENS_MIN, -1, SENS_STEP, SENS_MIN, SENS_MAX),
            SENS_MIN
        ));
        assert!(approx(stepped(1.0, 1, VOL_STEP, VOL_MIN, VOL_MAX), 1.0));
        assert!(approx(stepped(0.0, -1, VOL_STEP, VOL_MIN, VOL_MAX), 0.0));
    }

    #[test]
    fn stepped_moves_by_one_step() {
        assert!(approx(stepped(0.5, 1, VOL_STEP, VOL_MIN, VOL_MAX), 0.55));
        assert!(approx(stepped(0.5, -1, VOL_STEP, VOL_MIN, VOL_MAX), 0.45));
        assert!(approx(
            stepped(0.0022, 1, SENS_STEP, SENS_MIN, SENS_MAX),
            0.0027
        ));
    }

    #[test]
    fn quality_cycles_without_wrapping() {
        assert_eq!(cycle_quality(Quality::High, 1), Quality::High);
        assert_eq!(cycle_quality(Quality::High, -1), Quality::Medium);
        assert_eq!(cycle_quality(Quality::Medium, -1), Quality::Low);
        assert_eq!(cycle_quality(Quality::Low, -1), Quality::Low);
        assert_eq!(cycle_quality(Quality::Low, 1), Quality::Medium);
    }

    #[test]
    fn fov_step_preserves_sprint_gap() {
        let mut s = Settings::default();
        let gap = s.fov_run - s.fov;
        apply_step(&mut s, Field::Fov, 1);
        assert!(approx(s.fov_run - s.fov, gap));
        assert!(approx(s.fov, stepped(1.20, 1, FOV_STEP, FOV_MIN, FOV_MAX)));
    }

    #[test]
    fn apply_step_touches_only_the_target_value() {
        let mut s = Settings::default();
        apply_step(&mut s, Field::Master, -1);
        assert!(approx(s.master_volume, 0.95));
        assert!(approx(s.music_volume, Settings::default().music_volume));
        assert!(approx(s.sfx_volume, Settings::default().sfx_volume));
    }

    #[test]
    fn value_string_formats_per_field() {
        let s = Settings::default();
        assert_eq!(value_string(&s, Field::Master), "100%");
        assert_eq!(value_string(&s, Field::Music), "50%");
        assert_eq!(value_string(&s, Field::Quality), "High");
    }
}
