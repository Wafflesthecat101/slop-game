//! Carried lantern — a single warm spot light the player holds and can toggle.
//!
//! The lantern is attached as a **child of the player camera** (spawned by
//! [`crate::player`]), so it inherits the camera's position and look direction
//! for free and always points where the player is looking. Rather than reach
//! into the player controller, this plugin keys off the camera itself: an
//! observer fires when a `Camera3d` is added and hangs the light under it, and
//! the runtime systems read the camera's transform through a query. The player
//! module is never touched.
//!
//! Feel: a small **sway** is layered onto the lantern's local transform, driven
//! by the distance the camera has actually travelled (the same signal the
//! head-bob uses), so it lags and rocks like something held in the hand while
//! walking and settles when you stand still.
//!
//! Toggle: `F` turns it on/off. Since no day/night cycle is merged yet there is
//! nothing to read a "night" condition from, so the lantern simply **defaults
//! to on** — the sensible choice for a game about carrying light. When a
//! day/night resource lands, the default can key off it.
//!
//! Cost (per pillar P4): exactly **one** light, no shadow maps.

use crate::states::GameState;
use bevy::camera::Camera3d;
use bevy::ecs::lifecycle::Add;
use bevy::light::SpotLight;
use bevy::prelude::*;

/// Warm candle/flame tint.
const LANTERN_COLOR: Color = Color::srgb(1.0, 0.80, 0.52);
/// Luminous power (lumens) when lit; off is simply `0.0`.
const LANTERN_INTENSITY: f32 = 500_000.0;
/// How far the lantern throws light (metres).
const LANTERN_RANGE: f32 = 30.0;
/// Cone half-angles (radians): a soft pool of light ahead of the player.
const LANTERN_INNER_ANGLE: f32 = 0.35;
const LANTERN_OUTER_ANGLE: f32 = 0.65;

/// Resting offset of the light relative to the camera: down and to the right,
/// as if held at hip/hand height, and slightly forward.
const LANTERN_OFFSET: Vec3 = Vec3::new(0.35, -0.35, -0.15);

/// Radians of sway phase advanced per metre the camera travels. Matched to the
/// player's head-bob cadence so the lantern rocks in step with the walk.
const SWAY_FREQUENCY: f32 = 1.15;
/// Peak lateral/vertical translation of the sway (metres) — kept subtle.
const SWAY_AMPLITUDE: f32 = 0.05;
/// Peak yaw/pitch of the sway (radians) so the cone gently rocks, not just slides.
const SWAY_YAW: f32 = 0.05;
const SWAY_PITCH: f32 = 0.03;

pub struct LanternPlugin;

impl Plugin for LanternPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(attach_lantern).add_systems(
            Update,
            (toggle_lantern, sway_lantern).run_if(in_state(GameState::Playing)),
        );
    }
}

/// The carried light. Holds its on/off state and the sway bookkeeping.
#[derive(Component)]
struct Lantern {
    /// Whether the lantern is currently lit.
    on: bool,
    /// Accumulated sway phase (radians), advanced by distance walked.
    sway_phase: f32,
    /// Camera world position last frame, to measure distance travelled.
    prev_pos: Vec3,
}

/// When the player's camera is spawned, hang a single spot light under it.
fn attach_lantern(
    add: On<Add, Camera3d>,
    mut commands: Commands,
    cameras: Query<&Transform, With<Camera3d>>,
) {
    let camera = add.entity;
    let start = cameras
        .get(camera)
        .map(|t| t.translation)
        .unwrap_or_default();
    commands.entity(camera).with_child((
        Lantern {
            on: true,
            sway_phase: 0.0,
            prev_pos: start,
        },
        SpotLight {
            color: LANTERN_COLOR,
            intensity: LANTERN_INTENSITY,
            range: LANTERN_RANGE,
            inner_angle: LANTERN_INNER_ANGLE,
            outer_angle: LANTERN_OUTER_ANGLE,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(LANTERN_OFFSET),
    ));
}

/// `F` toggles the lantern by switching its intensity (keeps the single light
/// entity alive, just dark, which is cheaper and clearer than despawning it).
fn toggle_lantern(
    keys: Res<ButtonInput<KeyCode>>,
    lantern: Single<(&mut Lantern, &mut SpotLight)>,
) {
    if !keys.just_pressed(KeyCode::KeyF) {
        return;
    }
    let (mut lantern, mut light) = lantern.into_inner();
    lantern.on = !lantern.on;
    light.intensity = if lantern.on { LANTERN_INTENSITY } else { 0.0 };
}

/// Layer a subtle sway onto the lantern's local transform, driven by how far
/// the camera has moved (the head-bob signal), so it feels handheld. Because
/// the offset is rebuilt from a phase every frame it never drifts, and it
/// settles to rest when the player stands still.
fn sway_lantern(
    camera: Single<&Transform, (With<Camera3d>, Without<Lantern>)>,
    lantern: Single<(&mut Transform, &mut Lantern)>,
) {
    let cam_pos = camera.translation;
    let (mut transform, mut lantern) = lantern.into_inner();

    let delta = cam_pos - lantern.prev_pos;
    let travelled = Vec2::new(delta.x, delta.z).length();
    lantern.prev_pos = cam_pos;
    lantern.sway_phase += travelled * SWAY_FREQUENCY;

    let phase = lantern.sway_phase;
    let offset = Vec3::new(
        phase.sin() * SWAY_AMPLITUDE,
        (phase * 2.0).sin() * SWAY_AMPLITUDE * 0.5,
        0.0,
    );
    *transform =
        Transform::from_translation(LANTERN_OFFSET + offset).with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            phase.sin() * SWAY_YAW,
            (phase * 2.0).sin() * SWAY_PITCH,
            0.0,
        ));
}
