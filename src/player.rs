//! First-person player: mouse look, WASD movement, gravity, jump, climbing
//! steep slopes, and ground-following against the shared [`crate::terrain`]
//! heightfield.
//!
//! The player is a single camera entity carrying a [`Player`] component that
//! stores its look angles and vertical velocity. Keeping all player state on
//! one component (rather than spread across resources) makes the movement
//! system a single, self-contained query.

use crate::settings::Settings;
use crate::states::GameState;
use crate::terrain;
use crate::world::SKY_COLOR;
use bevy::camera::Hdr;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::window::{
    CursorGrabMode, CursorLeft, CursorOptions, PrimaryWindow, Window, WindowFocused,
};

const EYE_HEIGHT: f32 = 1.8;
const WALK_SPEED: f32 = 12.0;
const RUN_SPEED: f32 = 26.0;
const GRAVITY: f32 = 24.0;
const JUMP_SPEED: f32 = 9.0;

/// How quickly horizontal velocity approaches the target (1/s). Higher = more
/// responsive/snappy; lower = more floaty. Tuned for weighty-but-crisp feel.
const ACCEL: f32 = 12.0;
/// Head-bob amount (metres) and cadence (radians travelled per metre walked).
const BOB_AMPLITUDE: f32 = 0.09;
const BOB_FREQUENCY: f32 = 1.15;
/// How fast the FOV eases toward its walk/sprint target (1/s). The target
/// values themselves now live in [`Settings`].
const FOV_LERP: f32 = 8.0;

/// Horizontal radius of the player's body, used for collision push-out.
const PLAYER_RADIUS: f32 = 0.5;

/// Terrain whose upward-normal component (`terrain::normal(..).y`) is below this
/// engages deliberate climbing: on it you slide gently unless you hold the climb
/// key. Chosen so only the genuinely steepest terrain qualifies — gentle rolling
/// hills still walk exactly as before.
const STEEP_NORMAL_Y: f32 = 0.92;
/// Width (in normal-`y`) of the smoothstep ramp below [`STEEP_NORMAL_Y`] over
/// which the climb/slide response fades in, so steepness is never a hard on/off.
const CLIMB_BAND: f32 = 0.05;
/// Speed (m/s) travelled up a slope face while deliberately climbing — slow and
/// steady, far below [`WALK_SPEED`], so ascent always feels controlled.
const CLIMB_SPEED: f32 = 5.0;
/// Gentle downhill drift speed (m/s) on steep ground when not climbing — the
/// always-available, no-fail way back down.
const SLIDE_SPEED: f32 = 6.0;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_player, grab_cursor))
            .add_systems(
                Update,
                (
                    // Sync the cursor grab state to reality *before* the systems
                    // that read it, so releasing the cursor stops mouse-look on
                    // the same frame it happens (no one-frame lag).
                    (grab_on_click, release_cursor),
                    (mouse_look, move_player, sprint_fov),
                )
                    .chain()
                    // Player control and look only run during active gameplay;
                    // pausing (or the menu) freezes the camera.
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

/// A cylindrical (horizontal-circle) obstacle the player cannot walk through.
/// Attached to trees, rocks and beacon pillars; the player is pushed out of
/// any it overlaps. Kept trivially simple — no physics engine, just circles.
#[derive(Component)]
pub struct Collider {
    /// Horizontal radius in metres.
    pub radius: f32,
}

#[derive(Component)]
pub struct Player {
    /// Left/right look angle (radians).
    yaw: f32,
    /// Up/down look angle (radians), clamped to avoid flipping over.
    pitch: f32,
    /// Current vertical velocity (m/s); negative is falling.
    vertical_velocity: f32,
    /// Smoothed horizontal velocity (XZ, m/s) — gives movement weight/inertia
    /// instead of instant start/stop.
    velocity: Vec3,
    /// Distance-walked accumulator driving the head-bob sine wave.
    bob_phase: f32,
    /// Whether the player is sprinting this frame (read by `sprint_fov`).
    sprinting: bool,
}

fn spawn_player(mut commands: Commands) {
    let start = Vec3::new(0.0, terrain::height(0.0, 0.0) + EYE_HEIGHT, 0.0);
    commands.spawn((
        Camera3d::default(),
        // HDR + bloom make the glowing beacon orbs (see `world.rs`) actually
        // bloom, and give the scene a soft, filmic look.
        Hdr,
        Bloom::NATURAL,
        // Distance fog coloured like the sky masks the terrain's hard edge at
        // the far plane and adds aerial-perspective depth cues. Slightly
        // sun-tinted haze in the sun direction for atmosphere.
        DistanceFog {
            color: SKY_COLOR,
            directional_light_color: Color::srgb(1.0, 0.95, 0.85),
            directional_light_exponent: 30.0,
            falloff: FogFalloff::Linear {
                start: 120.0,
                end: 320.0,
            },
        },
        Transform::from_translation(start),
        Player {
            yaw: 0.0,
            pitch: 0.0,
            vertical_velocity: 0.0,
            velocity: Vec3::ZERO,
            bob_phase: 0.0,
            sprinting: false,
        },
    ));
}

fn grab_cursor(mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>) {
    cursor.visible = false;
    cursor.grab_mode = CursorGrabMode::Locked;
}

/// Re-grab the mouse when the player clicks inside the window (a user gesture,
/// which browsers require to enter pointer lock).
fn grab_on_click(
    mouse: Res<ButtonInput<MouseButton>>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if mouse.just_pressed(MouseButton::Left) && cursor.grab_mode == CursorGrabMode::None {
        cursor.visible = false;
        cursor.grab_mode = CursorGrabMode::Locked;
    }
}

/// Release the mouse cursor. Freeing must happen for several independent
/// reasons, and handling them all here is what makes a *single* Escape
/// reliably work across native and web:
///
/// * Escape pressed (native, and web when the key reaches us).
/// * The window loses focus.
/// * The cursor leaves the window. On the web the browser exits pointer lock
///   on the first Escape *itself* and usually swallows that key event, and it
///   does **not** drop window focus (only pointer lock is lost), so neither of
///   the above fires — our `grab_mode` would stay `Locked` and mouse-look would
///   keep following the now-free cursor (this is the reported bug). Winit does
///   not write the external unlock back to `CursorOptions` (it treats
///   `grab_mode` as our *intent*, see its `attempt_grab`), so we detect it via
///   the `CursorLeft` event the freed cursor produces and sync our state to
///   reality, stopping mouse-look immediately.
fn release_cursor(
    keys: Res<ButtonInput<KeyCode>>,
    mut focus: MessageReader<WindowFocused>,
    mut left: MessageReader<CursorLeft>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let lost_focus = focus.read().any(|e| !e.focused);
    let cursor_left = left.read().next().is_some();
    if keys.just_pressed(KeyCode::Escape) || lost_focus || cursor_left {
        cursor.visible = true;
        cursor.grab_mode = CursorGrabMode::None;
    }
}

fn mouse_look(
    motion: Res<AccumulatedMouseMotion>,
    settings: Res<Settings>,
    window: Single<(&Window, &CursorOptions), With<PrimaryWindow>>,
    mut player: Single<(&mut Player, &mut Transform)>,
) {
    let (window, cursor) = &*window;
    // Only steer the camera while the cursor is actually captured *and* the
    // window is focused — otherwise a freed or background cursor would still
    // drive the view.
    if cursor.grab_mode == CursorGrabMode::None || !window.focused {
        return;
    }
    let (player, transform) = &mut *player;
    player.yaw -= motion.delta.x * settings.mouse_sensitivity;
    player.pitch = (player.pitch - motion.delta.y * settings.mouse_sensitivity).clamp(-1.54, 1.54); // just under +/- 90 degrees
    transform.rotation = Quat::from_euler(EulerRot::YXZ, player.yaw, player.pitch, 0.0);
}

/// How strongly a slope with upward-normal component `normal_y` engages the
/// climb/slide system: `0.0` on walkable ground, ramping up to `1.0` on the
/// steepest climbable face. Steepness is derived purely from the `y` of
/// [`crate::terrain::normal`]; ground at or above [`STEEP_NORMAL_Y`] walks
/// normally (returns `0.0`), and the response fades in with a smoothstep over
/// [`CLIMB_BAND`] so climbing is never a hard on/off. Multiply by
/// [`CLIMB_SPEED`] for the actual controlled ascent speed.
fn climb_engagement(normal_y: f32) -> f32 {
    let t = ((STEEP_NORMAL_Y - normal_y) / CLIMB_BAND).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn move_player(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut player: Single<(&mut Player, &mut Transform)>,
    colliders: Query<(&Transform, &Collider), Without<Player>>,
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
    player.sprinting = keys.pressed(KeyCode::ShiftLeft) && wish != Vec3::ZERO;
    let speed = if player.sprinting {
        RUN_SPEED
    } else {
        WALK_SPEED
    };

    // Ease the horizontal velocity toward the desired velocity instead of
    // snapping to it — this is what gives movement weight (a short spin-up on
    // start and a glide on release) without a physics engine.
    let mut target = wish.normalize_or_zero() * speed;

    // Climbing / sliding on steep terrain. `terrain::normal` tells us how steep
    // the ground is; only genuinely steep faces engage (see `climb_engagement`).
    // There you either deliberately climb (hold the climb key while pushing into
    // the slope) at a slow controlled rate, or — the always-available, no-fail
    // escape — drift gently back downhill. The response is blended in by
    // steepness so the transition from ordinary walking is seamless.
    let normal = terrain::normal(transform.translation.x, transform.translation.z);
    let engage = climb_engagement(normal.y);
    let steep = engage > 0.0;
    if steep {
        let uphill = Vec2::new(-normal.x, -normal.z).normalize_or_zero();
        let wish_uphill = Vec2::new(wish.x, wish.z).normalize_or_zero().dot(uphill);
        let slope_target = if keys.pressed(KeyCode::Space) && wish_uphill > 0.0 {
            uphill * (CLIMB_SPEED * wish_uphill)
        } else {
            Vec2::new(normal.x, normal.z).normalize_or_zero() * SLIDE_SPEED
        };
        target = target.lerp(Vec3::new(slope_target.x, 0.0, slope_target.y), engage);
    }

    let t = (ACCEL * dt).min(1.0);
    player.velocity = player.velocity.lerp(target, t);
    transform.translation += player.velocity * dt;

    // Horizontal collision: push the player out of any obstacle circle it now
    // overlaps, and cancel the velocity component pointing into it so movement
    // slides along the surface instead of sticking or tunnelling through.
    let mut here = Vec2::new(transform.translation.x, transform.translation.z);
    for (obstacle, collider) in &colliders {
        let center = Vec2::new(obstacle.translation.x, obstacle.translation.z);
        let min_dist = collider.radius + PLAYER_RADIUS;
        let offset = here - center;
        let dist = offset.length();
        if dist < min_dist {
            let push = if dist > 1e-4 {
                offset / dist
            } else {
                Vec2::X // Degenerate exact-overlap: pick an arbitrary direction.
            };
            here = center + push * min_dist;
            let vel = Vec2::new(player.velocity.x, player.velocity.z);
            let into = vel.dot(push);
            if into < 0.0 {
                let slide = vel - push * into;
                player.velocity.x = slide.x;
                player.velocity.z = slide.y;
            }
        }
    }
    transform.translation.x = here.x;
    transform.translation.z = here.y;

    // Gravity + jump against the terrain surface.
    let ground = terrain::height(transform.translation.x, transform.translation.z) + EYE_HEIGHT;
    let grounded = transform.translation.y <= ground + 0.05;

    if grounded {
        player.vertical_velocity = 0.0;
        transform.translation.y = ground;
        // On steep, climbable ground Space means "climb" (handled above), so it
        // must not also fire a jump; you jump only from ordinary footing.
        if !steep && keys.just_pressed(KeyCode::Space) {
            player.vertical_velocity = JUMP_SPEED;
        }
    } else {
        player.vertical_velocity -= GRAVITY * dt;
    }
    transform.translation.y += player.vertical_velocity * dt;
    if transform.translation.y < ground {
        transform.translation.y = ground;
    }

    // Head bob: advance a phase by distance actually travelled and add a small
    // vertical sine offset while grounded. Because the offset is re-derived
    // from `ground` every frame it never accumulates or drifts.
    let ground_speed = player.velocity.length();
    player.bob_phase += ground_speed * BOB_FREQUENCY * dt;
    if grounded && player.vertical_velocity == 0.0 {
        let bob = (player.bob_phase).sin() * BOB_AMPLITUDE * (ground_speed / WALK_SPEED).min(1.0);
        transform.translation.y += bob;
    }
}

/// Smoothly widens the camera FOV while sprinting and eases it back when
/// walking — a classic, cheap "sense of speed" cue.
fn sprint_fov(
    time: Res<Time>,
    settings: Res<Settings>,
    player: Single<&Player>,
    mut projection: Single<&mut Projection, With<Camera3d>>,
) {
    if let Projection::Perspective(persp) = projection.as_mut() {
        let target = if player.sprinting {
            settings.fov_run
        } else {
            settings.fov
        };
        persp.fov = persp
            .fov
            .lerp(target, (FOV_LERP * time.delta_secs()).min(1.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain;

    #[test]
    fn flat_ground_never_climbs() {
        // A perfectly flat surface (normal straight up) and anything at or above
        // the steepness threshold walks normally — no climb/slide engagement.
        assert_eq!(climb_engagement(1.0), 0.0);
        assert_eq!(climb_engagement(STEEP_NORMAL_Y), 0.0);
        assert_eq!(climb_engagement(STEEP_NORMAL_Y + 0.01), 0.0);
    }

    #[test]
    fn steep_ground_engages_and_ramps_in() {
        // Just past the threshold the response is still (near) zero, then it
        // grows monotonically as the slope steepens (smaller normal.y).
        let just_steep = climb_engagement(STEEP_NORMAL_Y - 0.001);
        assert!(just_steep > 0.0 && just_steep < 0.05);
        let mid = climb_engagement(STEEP_NORMAL_Y - CLIMB_BAND * 0.5);
        let steep = climb_engagement(STEEP_NORMAL_Y - CLIMB_BAND);
        assert!(steep > mid && mid > just_steep);
    }

    #[test]
    fn engagement_is_bounded_and_saturates() {
        // Never leaves 0..=1, and the steepest possible ground saturates at 1
        // so the controlled climb speed can never be exceeded.
        for i in 0..=100 {
            let e = climb_engagement(i as f32 / 100.0);
            assert!((0.0..=1.0).contains(&e));
        }
        assert_eq!(climb_engagement(0.0), 1.0);
        assert_eq!(climb_engagement(STEEP_NORMAL_Y - CLIMB_BAND), 1.0);
    }

    #[test]
    fn climb_is_reachable_on_this_terrain() {
        // The feature must actually engage somewhere on the map: the steepest
        // terrain present should yield a substantial climb response, otherwise
        // the threshold is mistuned and climbing is dead code.
        let reach = terrain::HALF_SIZE - 5.0;
        let mut best = 0.0f32;
        let mut x = -reach;
        while x <= reach {
            let mut z = -reach;
            while z <= reach {
                best = best.max(climb_engagement(terrain::normal(x, z).y));
                z += 2.0;
            }
            x += 2.0;
        }
        assert!(
            best > 0.5,
            "steepest terrain barely climbs (engagement {best})"
        );
    }
}
