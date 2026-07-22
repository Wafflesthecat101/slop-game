//! World-reawakening: the atmosphere responds to how much light you've carried
//! back into the world.
//!
//! As shrines are rekindled the shared [`Progress`](crate::beacons::Progress)
//! climbs, and this plugin reads it (never writes it) to gradually lift the
//! world out of a muted, foggy slumber into a clear, warm and vivid day. It
//! drives three atmosphere channels from a single `0.0..=1.0` fraction:
//!
//! * the camera's [`DistanceFog`] — colour + how far you can see (density),
//! * the scene's [`GlobalAmbientLight`] — colour + brightness,
//! * a global colour/saturation grade via the camera's [`ColorGrading`].
//!
//! Deliberately **out of scope** (owned by the day/night plugin, #11): the
//! `DirectionalLight` sun and the `ClearColor` sky. This plugin only touches
//! fog, ambient and colour grading so the two can evolve without fighting over
//! the same components.
//!
//! The mapping from fraction to look is the pure, allocation-free
//! [`look_for`] function (unit-tested below, in the spirit of
//! [`crate::terrain`]); the ECS systems just smooth the fraction over time and
//! copy the result onto the live components, so the transition eases in rather
//! than snapping.

use crate::beacons::Progress;
use crate::states::GameState;
use bevy::prelude::*;
use bevy::render::view::ColorGrading;

/// Exponential approach rate (1/s) of the smoothed fraction toward the real
/// [`Progress`] fraction. Low enough that rekindling a shrine visibly *eases*
/// the world brighter over roughly a second, rather than popping instantly.
const SMOOTH_RATE: f32 = 1.2;

pub struct ReawakenPlugin;

impl Plugin for ReawakenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldAwakening>().add_systems(
            Update,
            (ensure_color_grading, reawaken_world)
                .chain()
                .run_if(in_state(GameState::Playing)),
        );
    }
}

/// The smoothed reawakening fraction. Eased toward `Progress::fraction()` each
/// frame so the atmosphere transition is gradual, not instantaneous.
#[derive(Resource, Default)]
struct WorldAwakening {
    smoothed: f32,
}

/// The atmosphere look for a given reawakening fraction. Plain numbers so the
/// mapping can be unit-tested without a Bevy `World`; the systems convert the
/// colour arrays into [`Color`]s when applying them.
struct ReawakenLook {
    /// Distance-fog linear falloff near/far bounds (metres). Larger = you see
    /// further, i.e. less fog.
    fog_start: f32,
    fog_end: f32,
    /// Fog haze colour (linear RGB).
    fog_color: [f32; 3],
    /// Global ambient light colour (linear RGB) and brightness.
    ambient_color: [f32; 3],
    ambient_brightness: f32,
    /// Post-tonemap saturation multiplier (`1.0` = unchanged, `<1` muted).
    saturation: f32,
    /// Colour-grade temperature (`>0` warmer/redder, `<0` cooler/bluer).
    temperature: f32,
}

/// Map a reawakening fraction (clamped to `0.0..=1.0`) to the atmosphere look.
///
/// At `0.0` the world is muted and foggy — cool, desaturated, dim ambient and
/// a near fog wall. At `1.0` it is clear, warm and vivid — the fog recedes,
/// ambient light brightens and warms, and colours are pushed toward a sunny,
/// saturated grade. Everything in between is a straight interpolation, so the
/// look changes monotonically with progress.
fn look_for(fraction: f32) -> ReawakenLook {
    let t = fraction.clamp(0.0, 1.0);
    ReawakenLook {
        fog_start: lerp(15.0, 150.0, t),
        fog_end: lerp(80.0, 480.0, t),
        fog_color: lerp3([0.30, 0.33, 0.38], [0.80, 0.72, 0.55], t),
        ambient_color: lerp3([0.40, 0.45, 0.55], [1.00, 0.92, 0.78], t),
        ambient_brightness: lerp(120.0, 650.0, t),
        saturation: lerp(0.65, 1.25, t),
        temperature: lerp(-0.15, 0.25, t),
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        lerp(a[0], b[0], t),
        lerp(a[1], b[1], t),
        lerp(a[2], b[2], t),
    ]
}

/// Ensure the player camera carries a [`ColorGrading`] we can drive. `Camera3d`
/// does not require one, so we add a neutral default the first time the camera
/// exists; the `Without<ColorGrading>` filter makes this a no-op every frame
/// after.
fn ensure_color_grading(
    mut commands: Commands,
    cameras: Query<Entity, (With<Camera3d>, Without<ColorGrading>)>,
) {
    for camera in &cameras {
        commands.entity(camera).insert(ColorGrading::default());
    }
}

/// Ease the smoothed fraction toward the real [`Progress`] and copy the
/// resulting [`look_for`] look onto the fog, ambient light and colour grade.
fn reawaken_world(
    time: Res<Time>,
    progress: Res<Progress>,
    mut awakening: ResMut<WorldAwakening>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut fog: Single<&mut DistanceFog, With<Camera3d>>,
    mut grading: Query<&mut ColorGrading, With<Camera3d>>,
) {
    let target = progress.fraction();
    let step = (SMOOTH_RATE * time.delta_secs()).min(1.0);
    awakening.smoothed += (target - awakening.smoothed) * step;

    let look = look_for(awakening.smoothed);

    fog.color = Color::linear_rgb(look.fog_color[0], look.fog_color[1], look.fog_color[2]);
    fog.falloff = FogFalloff::Linear {
        start: look.fog_start,
        end: look.fog_end,
    };

    ambient.color = Color::linear_rgb(
        look.ambient_color[0],
        look.ambient_color[1],
        look.ambient_color[2],
    );
    ambient.brightness = look.ambient_brightness;

    if let Ok(mut grading) = grading.single_mut() {
        grading.global.post_saturation = look.saturation;
        grading.global.temperature = look.temperature;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fraction_is_clamped() {
        let below = look_for(-1.0);
        let at_zero = look_for(0.0);
        let above = look_for(2.0);
        let at_one = look_for(1.0);
        assert_eq!(below.ambient_brightness, at_zero.ambient_brightness);
        assert_eq!(above.ambient_brightness, at_one.ambient_brightness);
        assert_eq!(below.fog_end, at_zero.fog_end);
        assert_eq!(above.saturation, at_one.saturation);
    }

    #[test]
    fn full_progress_is_clearer_brighter_and_warmer() {
        let muted = look_for(0.0);
        let vivid = look_for(1.0);
        // Clearer: fog recedes (you see further) at full progress.
        assert!(vivid.fog_start > muted.fog_start);
        assert!(vivid.fog_end > muted.fog_end);
        // Brighter + more saturated.
        assert!(vivid.ambient_brightness > muted.ambient_brightness);
        assert!(vivid.saturation > muted.saturation);
        // Warmer: grade temperature rises and both fog + ambient gain red.
        assert!(vivid.temperature > muted.temperature);
        assert!(vivid.fog_color[0] > muted.fog_color[0]);
        assert!(vivid.ambient_color[0] > muted.ambient_color[0]);
    }

    #[test]
    fn look_interpolates_monotonically_with_progress() {
        let a = look_for(0.25);
        let b = look_for(0.75);
        assert!(a.fog_end < b.fog_end);
        assert!(a.ambient_brightness < b.ambient_brightness);
        assert!(a.saturation < b.saturation);
        // The midpoint lies strictly between the endpoints.
        let mid = look_for(0.5);
        assert!(mid.fog_end > look_for(0.0).fog_end);
        assert!(mid.fog_end < look_for(1.0).fog_end);
    }

    #[test]
    fn zero_progress_is_muted() {
        let muted = look_for(0.0);
        // Desaturated (below neutral) and cool (negative temperature) when the
        // world is still asleep.
        assert!(muted.saturation < 1.0);
        assert!(muted.temperature < 0.0);
    }
}
