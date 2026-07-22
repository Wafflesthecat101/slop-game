//! Shrines — the game's core loop and its navigation landmarks.
//!
//! Each shrine is a tall dark pillar topped with a slowly bobbing emissive
//! orb. Because they are tall and glow (via the camera's HDR + bloom) they
//! read as beacons from across the map, pulling the player toward them — the
//! classic open-world curiosity/leading-anchor design.
//!
//! A shrine starts **dormant** (a cool, dim orb). Walking close **rekindles**
//! it: rather than vanishing, the orb brightens and warms and its light spills
//! further, so the world visibly gains light as you explore. Rekindling is the
//! heart of Lumen's "carry the light back" identity.
//!
//! This module owns the **Phase-1 shared contract** that the rest of the game
//! reacts to (day/night, world-reawakening, save, HUD): the [`Progress`]
//! resource and the [`ShrineLit`] message. Other plugins should only *read*
//! `Progress` / listen for `ShrineLit`, never edit this file.
//!
//! The feature stays data-light: one shared pillar mesh + orb mesh, a seeded
//! RNG for deterministic placement on hilltops, and two small systems
//! (animate, rekindle). Only each orb's tiny material is per-shrine, so
//! rekindling one brightens just that orb.

use crate::player::Collider;
use crate::states::GameState;
use crate::terrain::{self, HALF_SIZE};
use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

/// How many shrines to place in the world (the long-term goal).
const SHRINE_COUNT: usize = 12;
/// Player must get within this horizontal distance to rekindle a shrine.
pub const REKINDLE_RADIUS: f32 = 4.0;
/// Shrines only spawn above this elevation, so they crown hilltops/vistas.
const MIN_ELEVATION: f32 = 6.0;
/// Keep shrines spread out so each is a distinct destination.
const MIN_SEPARATION: f32 = 45.0;
const ORB_HEIGHT: f32 = 7.0;

/// Emissive colour of a dormant orb — cool and dim so it reads as "asleep".
const DORMANT_EMISSIVE: LinearRgba = LinearRgba::rgb(0.05, 0.35, 0.5);
/// Emissive colour of a rekindled orb — bright and warm so it reads as "alive"
/// and blooms hard through the camera's HDR + bloom.
const LIT_EMISSIVE: LinearRgba = LinearRgba::rgb(6.0, 4.2, 1.6);

pub struct BeaconsPlugin;

impl Plugin for BeaconsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Progress { lit: 0, total: 0 })
            .add_message::<ShrineLit>()
            .add_message::<RekindleRequest>()
            .add_systems(Startup, spawn_shrines)
            .add_systems(
                Update,
                (animate_orbs, rekindle_shrines).run_if(in_state(GameState::Playing)),
            );
    }
}

/// Shared world progress: how many shrines have been rekindled out of the
/// total. This is the Phase-1 contract other plugins read to drive the
/// day/night cycle, world reawakening, HUD and save. `total` is filled in at
/// spawn time.
#[derive(Resource)]
pub struct Progress {
    pub lit: u32,
    pub total: u32,
}

impl Progress {
    /// Fraction of the world reawakened, in `0.0..=1.0`. Returns `0.0` before
    /// any shrines exist so consumers never divide by zero.
    ///
    /// Part of the Phase-1 shared contract: the day/night (#11) and
    /// world-reawakening (#12) plugins drive lighting/fog from this. Allowed to
    /// be unused until those land so this contract can be built independently.
    #[allow(dead_code)]
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.lit as f32 / self.total as f32
        }
    }
}

/// Fired when the player rekindles a shrine; the HUD and atmosphere plugins
/// listen for feedback.
#[derive(Message)]
pub struct ShrineLit;

/// Request from the interaction plugin ([`crate::interact`]) to rekindle a
/// specific dormant shrine — the deliberate "press E" trigger. [`beacons`]
/// owns the *apply* logic (mark lit, bump [`Progress`], brighten the orb, emit
/// [`ShrineLit`]) so proximity now only shows a prompt; it no longer lights
/// shrines on its own.
///
/// [`beacons`]: crate::beacons
#[derive(Message)]
pub struct RekindleRequest(pub Entity);

/// A shrine landmark. The glowing orb is a child entity. Starts `lit == false`
/// (dormant) and flips to `true` once rekindled — it is never despawned, so
/// its pillar collider stays solid throughout. Exposed so [`crate::interact`]
/// can find the nearest dormant shrine to prompt for.
#[derive(Component)]
pub struct Shrine {
    pub lit: bool,
    /// Stable identifier: the shrine's spawn index. Placement is deterministic
    /// (fixed-seed `pick_hilltops`), so this index is the same every run and is
    /// what the save system ([`crate::save`]) persists to remember which
    /// shrines have been rekindled.
    pub index: u32,
}

/// The floating orb child, animated by `animate_orbs`. Public so the save
/// system can filter for orb materials when re-lighting loaded shrines.
#[derive(Component)]
pub struct Orb {
    /// Per-orb phase offset so they don't all bob in lockstep.
    phase: f32,
}

fn spawn_shrines(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut progress: ResMut<Progress>,
) {
    let pillar_mesh = meshes.add(Cuboid::new(0.6, ORB_HEIGHT, 0.6));
    let orb_mesh = meshes.add(Sphere::new(0.8));

    let pillar_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.08, 0.10),
        perceptual_roughness: 0.4,
        ..default()
    });

    let positions = pick_hilltops();
    progress.total = positions.len() as u32;

    for (index, base) in positions.into_iter().enumerate() {
        // Each shrine gets its own dormant orb material so rekindling one can
        // brighten just that orb (the mesh is still shared; only the tiny
        // material is per-shrine).
        let orb_mat = materials.add(StandardMaterial {
            base_color: Color::BLACK,
            emissive: DORMANT_EMISSIVE,
            ..default()
        });
        commands
            .spawn((
                Shrine {
                    lit: false,
                    index: index as u32,
                },
                Transform::from_translation(base),
                Visibility::default(),
                Mesh3d(pillar_mesh.clone()),
                MeshMaterial3d(pillar_mat.clone()),
                // Solid pillar you must walk around. Shrines are never
                // despawned, so this collider persists after rekindling.
                Collider { radius: 0.6 },
            ))
            .with_children(|shrine| {
                // Orb floats above the pillar top; child transform is local.
                shrine.spawn((
                    Orb {
                        phase: base.x + base.z,
                    },
                    Mesh3d(orb_mesh.clone()),
                    MeshMaterial3d(orb_mat),
                    Transform::from_xyz(0.0, ORB_HEIGHT * 0.5 + 1.5, 0.0),
                    // A dim, cool point light while dormant; rekindling turns
                    // it up and warms it so the orb spills real light around.
                    PointLight {
                        color: Color::srgb(0.4, 0.8, 1.0),
                        intensity: 60_000.0,
                        range: 24.0,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                ));
            });
    }
}

/// Deterministically choose spread-out hilltop positions for the shrines.
fn pick_hilltops() -> Vec<Vec3> {
    let mut rng = StdRng::seed_from_u64(0xB3AC0Au64);
    let reach = HALF_SIZE - 10.0;
    let mut chosen: Vec<Vec3> = Vec::with_capacity(SHRINE_COUNT);

    // Rejection sampling: keep candidates that are high, fairly flat, and far
    // enough from already-placed shrines. Bounded attempts so it always ends.
    for _ in 0..4000 {
        if chosen.len() == SHRINE_COUNT {
            break;
        }
        let x = rng.random_range(-reach..reach);
        let z = rng.random_range(-reach..reach);
        let y = terrain::height(x, z);
        if y < MIN_ELEVATION || terrain::normal(x, z).y < 0.9 {
            continue;
        }
        let pos = Vec3::new(x, y, z);
        if chosen.iter().any(|p| p.distance(pos) < MIN_SEPARATION) {
            continue;
        }
        chosen.push(pos);
    }
    chosen
}

/// Bob the orbs up/down and spin them slowly — cheap "aliveness" juice that
/// also makes the glow shimmer as it catches the light.
fn animate_orbs(time: Res<Time>, mut orbs: Query<(&Orb, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (orb, mut tf) in &mut orbs {
        let base_y = ORB_HEIGHT * 0.5 + 1.5;
        tf.translation.y = base_y + (t * 1.5 + orb.phase).sin() * 0.35;
        tf.rotate_y(time.delta_secs() * 0.8);
    }
}

/// Apply the deliberate rekindle triggered by [`crate::interact`]: for each
/// [`RekindleRequest`], if the named shrine is still dormant, brighten and warm
/// its orb, turn its light up, mark it lit, bump [`Progress`] and emit
/// [`ShrineLit`]. The shrine stays in the world. The proximity + key-press
/// *trigger* lives in [`crate::interact`]; this system owns only the *apply*
/// half of the contract, so downstream plugins (HUD, day/night, reawaken) are
/// unchanged.
fn rekindle_shrines(
    mut requests: MessageReader<RekindleRequest>,
    mut progress: ResMut<Progress>,
    mut lit_writer: MessageWriter<ShrineLit>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut shrines: Query<(&mut Shrine, &Children)>,
    orbs: Query<&MeshMaterial3d<StandardMaterial>, With<Orb>>,
    mut lights: Query<&mut PointLight>,
) {
    for RekindleRequest(entity) in requests.read() {
        let Ok((mut shrine, children)) = shrines.get_mut(*entity) else {
            continue;
        };
        if light_shrine(
            &mut shrine,
            children,
            &mut progress,
            &mut materials,
            &orbs,
            &mut lights,
        ) {
            lit_writer.write(ShrineLit);
        }
    }
}

/// Mark a shrine lit and apply its rekindled look — brighten and warm the orb
/// material and turn its point light up — bumping [`Progress`]. Returns `true`
/// if the shrine was newly lit, `false` if it was already lit (idempotent).
///
/// Shared by the player rekindle (`rekindle_shrines`, which then emits
/// [`ShrineLit`]) and the save system's load-time hook ([`crate::save`], which
/// applies persisted progress *silently* — no `ShrineLit`, so loading a saved
/// world replays no SFX or feedback).
pub fn light_shrine(
    shrine: &mut Shrine,
    children: &Children,
    progress: &mut Progress,
    materials: &mut Assets<StandardMaterial>,
    orbs: &Query<&MeshMaterial3d<StandardMaterial>, With<Orb>>,
    lights: &mut Query<&mut PointLight>,
) -> bool {
    if shrine.lit {
        return false;
    }

    shrine.lit = true;
    progress.lit += 1;

    for child in children.iter() {
        if let Ok(material) = orbs.get(child)
            && let Some(mut mat) = materials.get_mut(&material.0)
        {
            mat.emissive = LIT_EMISSIVE;
        }
        if let Ok(mut light) = lights.get_mut(child) {
            light.color = Color::srgb(1.0, 0.85, 0.55);
            light.intensity = 320_000.0;
            light.range = 34.0;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fraction_is_zero_before_any_shrines() {
        let p = Progress { lit: 0, total: 0 };
        assert_eq!(p.fraction(), 0.0);
    }

    #[test]
    fn fraction_tracks_lit_over_total() {
        let p = Progress { lit: 3, total: 12 };
        assert!((p.fraction() - 0.25).abs() < 1e-6);
        let done = Progress { lit: 12, total: 12 };
        assert_eq!(done.fraction(), 1.0);
    }
}
