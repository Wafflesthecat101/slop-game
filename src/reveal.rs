//! Lantern-revealed hidden objects — depth for the carried lantern (pillars
//! P2/P3): a class of dormant "wisps" that only fade into view while the
//! player actively lights them, gently rewarding exploration.
//!
//! ## The reveal rule
//!
//! A hidden object is *revealed* when **all three** hold ([`is_revealed`]):
//!
//! 1. **Lantern-on** — the carried lantern is currently lit.
//! 2. **Proximity** — the object is within [`REVEAL_RADIUS`] metres of the
//!    player camera.
//! 3. **Facing** — the object lies inside the pool of light ahead of the
//!    player: the dot product between the camera's forward direction and the
//!    (normalised) direction to the object is at least [`REVEAL_MIN_DOT`].
//!
//! It is deliberately **cheap** (pillar P4): one distance check and one
//! dot-product per object — no raycasting, no shadow queries. And it is
//! **calm** (pillar P1): missing a wisp costs nothing; it simply fades back
//! out. There is no timer, score or fail state.
//!
//! We do **not** reach into [`crate::lantern`]. The lantern is the scene's
//! single [`SpotLight`] and rides under the camera, so we read "lantern-on"
//! straight from that light's `intensity` (it is set to `0.0` when toggled
//! off) and read the player's position/facing from the [`Camera3d`] transform.
//! No new lights or cameras are spawned — reveal is pure material alpha — so
//! the CI light/camera invariants are untouched.

use crate::states::GameState;
use crate::terrain;
use bevy::camera::Camera3d;
use bevy::light::SpotLight;
use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

/// How many hidden wisps to scatter across the world.
const WISP_COUNT: usize = 40;
/// Half-extent of the square band the wisps are scattered within (metres).
const SCATTER_HALF: f32 = 180.0;
/// Height above the ground a wisp floats at (metres).
const WISP_HOVER: f32 = 1.4;
/// Visible radius of a wisp (metres).
const WISP_RADIUS: f32 = 0.35;

/// How close (metres) the object must be to the player to be revealed.
const REVEAL_RADIUS: f32 = 18.0;
/// Minimum dot product between the camera's forward and the direction to the
/// object for it to count as "in the lantern's pool of light ahead". `0.6` is
/// a cone a touch wider than the lantern's own light cone, so an object at the
/// edge of the beam still fades in.
const REVEAL_MIN_DOT: f32 = 0.6;
/// A `SpotLight` brighter than this counts as "the lantern is on" (it is set
/// to exactly `0.0` when toggled off).
const LANTERN_ON_INTENSITY: f32 = 1.0;
/// Exponential approach rate (1/s) of a wisp's fade toward its target, so it
/// eases in/out over roughly a third of a second rather than popping.
const FADE_RATE: f32 = 4.0;

/// Soft colour a revealed wisp glows (linear RGB, boosted so the camera's
/// HDR + bloom picks it up like the shrine orbs do).
const WISP_EMISSIVE: LinearRgba = LinearRgba::rgb(0.6, 2.4, 3.0);
const WISP_BASE_COLOR: Color = Color::srgb(0.45, 0.85, 0.95);

pub struct RevealPlugin;

impl Plugin for RevealPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_wisps)
            .add_systems(Update, reveal_wisps.run_if(in_state(GameState::Playing)));
    }
}

/// A hidden wisp. Holds its current fade (`0.0` invisible … `1.0` fully lit),
/// which is written onto its own material's alpha each frame.
#[derive(Component)]
struct HiddenWisp {
    fade: f32,
}

/// Whether a hidden object should currently be visible — the whole reveal rule
/// in one cheap, pure predicate (distance + dot product + on/off), unit-tested
/// below without a Bevy `World`. `cam_forward` is expected to be normalised.
fn is_revealed(
    cam_pos: Vec3,
    cam_forward: Vec3,
    obj_pos: Vec3,
    lantern_on: bool,
    radius: f32,
    min_dot: f32,
) -> bool {
    if !lantern_on {
        return false;
    }
    let to_obj = obj_pos - cam_pos;
    let dist = to_obj.length();
    if dist > radius {
        return false;
    }
    // Right on top of the player: no meaningful direction, treat as revealed.
    if dist < 1e-3 {
        return true;
    }
    (to_obj / dist).dot(cam_forward) >= min_dot
}

/// Scatter the wisps at startup. Each gets its **own** translucent material
/// (starting fully transparent) so it can fade independently; the shared
/// sphere mesh keeps them cheap.
fn spawn_wisps(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Sphere::new(WISP_RADIUS));
    let mut rng = StdRng::seed_from_u64(0x01E5_2EED);

    for _ in 0..WISP_COUNT {
        let x = rng.random_range(-SCATTER_HALF..SCATTER_HALF);
        let z = rng.random_range(-SCATTER_HALF..SCATTER_HALF);
        let y = terrain::height(x, z) + WISP_HOVER;

        let material = materials.add(StandardMaterial {
            base_color: WISP_BASE_COLOR.with_alpha(0.0),
            emissive: WISP_EMISSIVE,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });

        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(Vec3::new(x, y, z)),
            HiddenWisp { fade: 0.0 },
        ));
    }
}

/// Fade each wisp toward revealed/hidden based on the lantern and the player's
/// position/facing, writing the eased fade onto the wisp's material alpha.
fn reveal_wisps(
    time: Res<Time>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    lanterns: Query<&SpotLight>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut wisps: Query<(
        &GlobalTransform,
        &MeshMaterial3d<StandardMaterial>,
        &mut HiddenWisp,
    )>,
) {
    let cam_pos = camera.translation();
    let cam_forward = *camera.forward();
    let lantern_on = lanterns
        .iter()
        .any(|light| light.intensity >= LANTERN_ON_INTENSITY);

    let step = (FADE_RATE * time.delta_secs()).min(1.0);

    for (transform, material, mut wisp) in &mut wisps {
        let target = if is_revealed(
            cam_pos,
            cam_forward,
            transform.translation(),
            lantern_on,
            REVEAL_RADIUS,
            REVEAL_MIN_DOT,
        ) {
            1.0
        } else {
            0.0
        };

        wisp.fade += (target - wisp.fade) * step;

        if let Some(mut material) = materials.get_mut(&material.0) {
            material.base_color = WISP_BASE_COLOR.with_alpha(wisp.fade);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FWD: Vec3 = Vec3::NEG_Z; // Bevy's camera looks down -Z.

    #[test]
    fn hidden_when_lantern_off() {
        // Perfectly in range and facing, but the lantern is off → hidden.
        let obj = Vec3::new(0.0, 0.0, -5.0);
        assert!(!is_revealed(
            Vec3::ZERO,
            FWD,
            obj,
            false,
            REVEAL_RADIUS,
            REVEAL_MIN_DOT
        ));
    }

    #[test]
    fn revealed_when_on_close_and_facing() {
        let obj = Vec3::new(0.0, 0.0, -5.0);
        assert!(is_revealed(
            Vec3::ZERO,
            FWD,
            obj,
            true,
            REVEAL_RADIUS,
            REVEAL_MIN_DOT
        ));
    }

    #[test]
    fn hidden_when_too_far() {
        let obj = Vec3::new(0.0, 0.0, -(REVEAL_RADIUS + 1.0));
        assert!(!is_revealed(
            Vec3::ZERO,
            FWD,
            obj,
            true,
            REVEAL_RADIUS,
            REVEAL_MIN_DOT
        ));
    }

    #[test]
    fn hidden_when_facing_away() {
        // Object is behind the player (toward +Z) but the camera looks -Z.
        let obj = Vec3::new(0.0, 0.0, 5.0);
        assert!(!is_revealed(
            Vec3::ZERO,
            FWD,
            obj,
            true,
            REVEAL_RADIUS,
            REVEAL_MIN_DOT
        ));
    }

    #[test]
    fn hidden_when_off_to_the_side() {
        // Within radius but ~90° off the forward axis → below the dot cutoff.
        let obj = Vec3::new(5.0, 0.0, 0.0);
        assert!(!is_revealed(
            Vec3::ZERO,
            FWD,
            obj,
            true,
            REVEAL_RADIUS,
            REVEAL_MIN_DOT
        ));
    }

    #[test]
    fn revealed_when_on_top_of_player() {
        // Degenerate zero-distance case must not divide by zero; treat as lit.
        assert!(is_revealed(
            Vec3::ZERO,
            FWD,
            Vec3::ZERO,
            true,
            REVEAL_RADIUS,
            REVEAL_MIN_DOT
        ));
    }
}
