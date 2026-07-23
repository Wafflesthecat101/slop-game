//! Water: a single translucent plane at a fixed sea level so the terrain's
//! lowlands read as lakes and coastline — a calm visual landmark (design
//! pillar P3), not a hazard.
//!
//! One flat quad is spawned once at [`Startup`] and sits at [`SEA_LEVEL`],
//! chosen against [`crate::terrain::height`] so a meaningful share of the
//! world's basins fall below it while the hills stay dry. The material is a
//! cheap low-roughness [`StandardMaterial`] with alpha blending: it picks up a
//! crisp specular glint from the existing sun for a "reflective" look without
//! any reflection probes or extra render passes (P4). A gentle vertical bob —
//! the pure [`surface_y`] helper, unit-tested below in the spirit of
//! [`crate::terrain`] — gives the surface a little life; the per-frame system
//! just writes one transform.

use crate::states::GameState;
use crate::terrain::HALF_SIZE;
use bevy::prelude::*;
use std::f32::consts::TAU;

/// World-space height of the still-water surface (metres). The terrain spans
/// roughly `-23..=23` m; at this level about a third of the map's basins sit
/// underwater, reading as lakes and coast while the hills stay dry.
pub const SEA_LEVEL: f32 = -4.0;

/// Peak vertical displacement of the gentle surface bob (metres).
const WAVE_AMPLITUDE: f32 = 0.12;

/// Real seconds for one full bob cycle. Slow enough to feel calm, not choppy.
const WAVE_SECONDS: f32 = 5.0;

pub struct WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_water)
            .add_systems(Update, animate_water.run_if(in_state(GameState::Playing)));
    }
}

/// Marks the single water-plane entity so the animation system can find it.
#[derive(Component)]
struct Water;

/// Height of the animated water surface at `seconds`, oscillating by
/// [`WAVE_AMPLITUDE`] around `sea_level`. Pure so the motion can be unit-tested
/// without an `App`; continuous in `seconds`, so the bob never pops.
fn surface_y(sea_level: f32, seconds: f32) -> f32 {
    sea_level + WAVE_AMPLITUDE * (seconds * TAU / WAVE_SECONDS).sin()
}

/// A cheap, translucent water material. Low roughness gives a crisp specular
/// reflection of the sun/sky for a "reflective" read without any reflection
/// probes; alpha blending lets the sandy lakebed show faintly through.
fn water_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgba(0.15, 0.34, 0.48, 0.72),
        perceptual_roughness: 0.08,
        metallic: 0.0,
        reflectance: 0.55,
        alpha_mode: AlphaMode::Blend,
        ..default()
    }
}

fn spawn_water(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // One flat quad spanning the whole world; the distance fog hides its edge.
    let mesh = meshes.add(
        Plane3d::default()
            .mesh()
            .size(HALF_SIZE * 2.0, HALF_SIZE * 2.0),
    );
    commands.spawn((
        Water,
        Mesh3d(mesh),
        MeshMaterial3d(materials.add(water_material())),
        Transform::from_xyz(0.0, SEA_LEVEL, 0.0),
    ));
}

/// Bobs the water plane on the [`surface_y`] curve. `Single` quietly no-ops if
/// the plane is absent and never spawns a second one.
fn animate_water(time: Res<Time>, water: Single<&mut Transform, With<Water>>) {
    let mut transform = water.into_inner();
    transform.translation.y = surface_y(SEA_LEVEL, time.elapsed_secs());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::{self, HALF_SIZE};

    #[test]
    fn surface_starts_at_sea_level_and_is_deterministic() {
        assert_eq!(surface_y(SEA_LEVEL, 0.0), SEA_LEVEL);
        assert_eq!(surface_y(SEA_LEVEL, 3.2), surface_y(SEA_LEVEL, 3.2));
    }

    #[test]
    fn surface_stays_within_wave_band() {
        let mut t = 0.0;
        while t < 2.0 * WAVE_SECONDS {
            let y = surface_y(SEA_LEVEL, t);
            assert!((y - SEA_LEVEL).abs() <= WAVE_AMPLITUDE + 1e-4);
            t += 0.05;
        }
        // A quarter period in, the surface is at its crest.
        let crest = surface_y(SEA_LEVEL, WAVE_SECONDS / 4.0);
        assert!((crest - (SEA_LEVEL + WAVE_AMPLITUDE)).abs() < 1e-4);
    }

    #[test]
    fn sea_level_submerges_lowlands_but_not_the_whole_map() {
        // Read the shared terrain height so the plane is guaranteed to sit
        // against real ground: some basins must fall below the surface (so
        // water is visible) while plenty of land stays dry.
        let mut total = 0;
        let mut submerged = 0;
        let mut x = -HALF_SIZE;
        while x <= HALF_SIZE {
            let mut z = -HALF_SIZE;
            while z <= HALF_SIZE {
                total += 1;
                if terrain::height(x, z) < SEA_LEVEL {
                    submerged += 1;
                }
                z += 4.0;
            }
            x += 4.0;
        }
        let fraction = submerged as f32 / total as f32;
        assert!(
            (0.05..0.6).contains(&fraction),
            "water covers {fraction} of the map — expected a fraction of lowlands"
        );
    }
}
