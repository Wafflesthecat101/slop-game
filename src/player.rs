//! First-person player: mouse look, WASD movement, gravity, jump, and
//! ground-following against the shared [`crate::terrain`] heightfield.
//!
//! The player is a single camera entity carrying a [`Player`] component that
//! stores its look angles and vertical velocity. Keeping all player state on
//! one component (rather than spread across resources) makes the movement
//! system a single, self-contained query.

use crate::terrain;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

const EYE_HEIGHT: f32 = 1.8;
const WALK_SPEED: f32 = 12.0;
const RUN_SPEED: f32 = 26.0;
const GRAVITY: f32 = 24.0;
const JUMP_SPEED: f32 = 9.0;
const MOUSE_SENSITIVITY: f32 = 0.0022;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_player, grab_cursor))
            .add_systems(Update, (mouse_look, move_player, toggle_cursor));
    }
}

#[derive(Component)]
pub struct Player {
    /// Left/right look angle (radians).
    yaw: f32,
    /// Up/down look angle (radians), clamped to avoid flipping over.
    pitch: f32,
    /// Current vertical velocity (m/s); negative is falling.
    vertical_velocity: f32,
}

fn spawn_player(mut commands: Commands) {
    let start = Vec3::new(0.0, terrain::height(0.0, 0.0) + EYE_HEIGHT, 0.0);
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(start),
        Player {
            yaw: 0.0,
            pitch: 0.0,
            vertical_velocity: 0.0,
        },
    ));
}

fn grab_cursor(mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>) {
    cursor.visible = false;
    cursor.grab_mode = CursorGrabMode::Locked;
}

/// Press Escape to release the mouse cursor (so the window can be closed /
/// the OS regained), click to re-grab it.
fn toggle_cursor(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        cursor.visible = true;
        cursor.grab_mode = CursorGrabMode::None;
    }
    if mouse.just_pressed(MouseButton::Left) {
        cursor.visible = false;
        cursor.grab_mode = CursorGrabMode::Locked;
    }
}

fn mouse_look(
    motion: Res<AccumulatedMouseMotion>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    mut player: Single<(&mut Player, &mut Transform)>,
) {
    if cursor.grab_mode == CursorGrabMode::None {
        return;
    }
    let (player, transform) = &mut *player;
    player.yaw -= motion.delta.x * MOUSE_SENSITIVITY;
    player.pitch = (player.pitch - motion.delta.y * MOUSE_SENSITIVITY).clamp(-1.54, 1.54); // just under +/- 90 degrees
    transform.rotation = Quat::from_euler(EulerRot::YXZ, player.yaw, player.pitch, 0.0);
}

fn move_player(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut player: Single<(&mut Player, &mut Transform)>,
) {
    let dt = time.delta_secs();
    let (player, transform) = &mut *player;

    // Horizontal movement basis comes straight from the camera's own
    // orientation via Bevy's `Transform::forward`/`right` helpers, flattened
    // onto the XZ plane so that looking up/down never changes walk speed or
    // direction. (`right` is already horizontal for a yaw+pitch rotation; only
    // `forward` tilts with pitch, hence the flatten + renormalize.)
    let forward = transform.forward().reject_from(Vec3::Y).normalize_or_zero();
    let right = transform.right().reject_from(Vec3::Y).normalize_or_zero();
    let mut wish = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        wish += forward;
    }
    if keys.pressed(KeyCode::KeyS) {
        wish -= forward;
    }
    if keys.pressed(KeyCode::KeyD) {
        wish += right;
    }
    if keys.pressed(KeyCode::KeyA) {
        wish -= right;
    }
    let speed = if keys.pressed(KeyCode::ShiftLeft) {
        RUN_SPEED
    } else {
        WALK_SPEED
    };
    transform.translation += wish.normalize_or_zero() * speed * dt;

    // Gravity + jump against the terrain surface.
    let ground = terrain::height(transform.translation.x, transform.translation.z) + EYE_HEIGHT;
    let grounded = transform.translation.y <= ground + 0.05;

    if grounded {
        player.vertical_velocity = 0.0;
        transform.translation.y = ground;
        if keys.just_pressed(KeyCode::Space) {
            player.vertical_velocity = JUMP_SPEED;
        }
    } else {
        player.vertical_velocity -= GRAVITY * dt;
    }
    transform.translation.y += player.vertical_velocity * dt;
    if transform.translation.y < ground {
        transform.translation.y = ground;
    }
}
