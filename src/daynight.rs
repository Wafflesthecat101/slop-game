//! Day/night cycle — the sky and sun slowly wheel through a day, and the
//! world *brightens as you rekindle shrines*.
//!
//! This plugin owns only two things: the **sun** (the single
//! [`DirectionalLight`] spawned by [`crate::world`]) and the **sky**
//! ([`ClearColor`]). It reads [`Progress`] to bias the cycle toward daylight —
//! the more shrines are lit, the higher the sun's arc rides and the shorter
//! and brighter the nights become, so exploration literally reawakens the
//! world into day. Fog and ambient light are owned by a sibling plugin and are
//! deliberately untouched here.
//!
//! All the lighting maths lives in the pure [`sky_state`] helper (mapping a
//! cycle phase + progress to a sun orientation/colour/intensity and a sky
//! colour), so it is unit-tested without spinning up an `App`. Because that
//! function is continuous in both inputs and the phase advances smoothly with
//! real time, the result eases between day and night with no popping. The
//! per-frame system is cheap: it reads two resources and writes one resource
//! plus two fields on one entity.

use crate::beacons::Progress;
use crate::states::GameState;
use bevy::prelude::*;
use std::f32::consts::TAU;

/// Real seconds for one full day→night→day cycle. Long enough that the motion
/// is a gentle drift rather than a strobe.
const CYCLE_SECONDS: f32 = 180.0;

/// Where the cycle sits at startup, so the game opens in a pleasant climbing
/// morning rather than at midnight (phase `0.0`).
const START_PHASE: f32 = 0.28;

/// Peak sun elevation (radians, ~66°). Kept below straight-up both so shadows
/// stay long and readable and so the look-at direction never degenerates.
const MAX_SUN_ELEVATION: f32 = 1.15;

/// Directional-light illuminance (lux) at deepest night vs. high noon. Night
/// is a dim cool "moonlight" floor; day matches the world's original warm sun.
const NIGHT_ILLUMINANCE: f32 = 120.0;
const DAY_ILLUMINANCE: f32 = 11_000.0;

pub struct DayNightPlugin;

impl Plugin for DayNightPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, drive_day_night.run_if(in_state(GameState::Playing)));
    }
}

/// A snapshot of the sky/sun for one instant of the cycle: everything the
/// per-frame system needs to write, computed purely so it can be unit-tested.
pub struct SkyState {
    /// Orientation for the sun entity's `Transform` (its forward is the
    /// direction the light travels).
    pub sun_rotation: Quat,
    pub sun_color: Color,
    pub sun_illuminance: f32,
    pub sky_color: Color,
}

/// Maps a cycle `phase` in `0.0..1.0` (0 = midnight, 0.5 = noon) and world
/// `progress` in `0.0..=1.0` to the sun orientation/colour/intensity and sky
/// colour.
///
/// `progress` biases the cycle toward daylight: it lifts the sun's altitude so
/// that at full progress the sun stays high and the world never truly darkens,
/// making "light more shrines → brighter world" fall out of a single, smooth
/// interpolation.
pub fn sky_state(phase: f32, progress: f32) -> SkyState {
    let azimuth = phase * TAU;

    // Raw altitude as the sine of the phase: -1 at midnight, +1 at noon.
    let raw_altitude = -(phase * TAU).cos();
    // Progress lifts the whole arc toward its peak (permanent noon at 1.0).
    let altitude = (raw_altitude * (1.0 - progress) + progress).clamp(-1.0, 1.0);

    // Smooth day/twilight/night blend from the sun's height above the horizon.
    let daylight = smoothstep(-0.25, 0.35, altitude);

    // Sun position on its arc; the light travels the opposite way (into the
    // scene), so the entity looks along `-sun_dir`.
    let elevation = altitude * MAX_SUN_ELEVATION;
    let (sin_e, cos_e) = elevation.sin_cos();
    let sun_dir = Vec3::new(cos_e * azimuth.cos(), sin_e, cos_e * azimuth.sin());
    let sun_rotation = Transform::default().looking_to(-sun_dir, Vec3::Y).rotation;

    let night_sun = Color::srgb(0.55, 0.66, 1.0).to_linear();
    let day_sun = Color::srgb(1.0, 0.93, 0.78).to_linear();
    let sun_color = Color::from(lerp_linear(night_sun, day_sun, daylight));

    let night_sky = Color::srgb(0.015, 0.02, 0.06).to_linear();
    let day_sky = crate::world::SKY_COLOR.to_linear();
    let sky_color = Color::from(lerp_linear(night_sky, day_sky, daylight));

    SkyState {
        sun_rotation,
        sun_color,
        sun_illuminance: lerp(NIGHT_ILLUMINANCE, DAY_ILLUMINANCE, daylight),
        sky_color,
    }
}

/// Drives the existing sun and the sky from the current cycle phase. Uses
/// `Single` so it quietly no-ops if the sun isn't present, and never spawns a
/// second light.
fn drive_day_night(
    time: Res<Time>,
    progress: Res<Progress>,
    mut clear_color: ResMut<ClearColor>,
    sun: Single<(&mut Transform, &mut DirectionalLight)>,
) {
    let phase = (START_PHASE + time.elapsed_secs() / CYCLE_SECONDS).fract();
    let state = sky_state(phase, progress.fraction());

    let (mut transform, mut light) = sun.into_inner();
    transform.rotation = state.sun_rotation;
    light.color = state.sun_color;
    light.illuminance = state.sun_illuminance;
    clear_color.0 = state.sky_color;
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_linear(a: LinearRgba, b: LinearRgba, t: f32) -> LinearRgba {
    LinearRgba::rgb(
        lerp(a.red, b.red, t),
        lerp(a.green, b.green, t),
        lerp(a.blue, b.blue, t),
    )
}

/// Hermite ease between `edge0` and `edge1` (matches GLSL `smoothstep`).
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sun's forward (the direction light travels) for a given state.
    fn light_forward(state: &SkyState) -> Vec3 {
        state.sun_rotation * Vec3::NEG_Z
    }

    #[test]
    fn noon_is_brighter_than_midnight() {
        let midnight = sky_state(0.0, 0.0);
        let noon = sky_state(0.5, 0.0);
        assert!(noon.sun_illuminance > midnight.sun_illuminance);
        // At noon the sun is above the horizon, so its light points downward.
        assert!(
            light_forward(&noon).y < 0.0,
            "noon sun should shine downward"
        );
    }

    #[test]
    fn progress_brightens_the_world() {
        // At a fixed night phase, lighting more shrines must raise both the
        // sun's intensity and the sky's brightness — the core acceptance.
        let dark = sky_state(0.0, 0.0);
        let dim = sky_state(0.0, 0.5);
        let bright = sky_state(0.0, 1.0);
        assert!(dim.sun_illuminance > dark.sun_illuminance);
        assert!(bright.sun_illuminance > dim.sun_illuminance);

        let sky_luma = |c: Color| {
            let l = c.to_linear();
            l.red + l.green + l.blue
        };
        assert!(sky_luma(bright.sky_color) > sky_luma(dark.sky_color));
    }

    #[test]
    fn outputs_are_finite_and_bounded() {
        let mut phase = 0.0;
        while phase < 1.0 {
            for &progress in &[0.0, 0.5, 1.0] {
                let s = sky_state(phase, progress);
                assert!(s.sun_illuminance.is_finite() && s.sun_illuminance >= 0.0);
                assert!((s.sun_rotation.length() - 1.0).abs() < 1e-3);
                for ch in s.sky_color.to_linear().to_f32_array() {
                    assert!(ch.is_finite() && (0.0..=2.0).contains(&ch));
                }
            }
            phase += 0.01;
        }
    }

    #[test]
    fn cycle_is_smooth_no_popping() {
        // Adjacent instants of the cycle (including across the midnight wrap)
        // must produce only small changes — no discontinuous "pop".
        let step = 0.001;
        let mut phase = 0.0;
        let mut prev = sky_state(phase, 0.3);
        while phase < 1.0 {
            phase += step;
            let cur = sky_state(phase.fract(), 0.3);
            assert!(
                (cur.sun_illuminance - prev.sun_illuminance).abs() < 200.0,
                "illuminance jumped at phase {phase}"
            );
            let ddir = (light_forward(&cur) - light_forward(&prev)).length();
            assert!(ddir < 0.05, "sun direction jumped at phase {phase}");
            prev = cur;
        }
    }
}
