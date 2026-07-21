//! Glowing beacons — the game's objective and its navigation landmarks.
//!
//! Each beacon is a tall dark pillar topped with a bright, slowly bobbing
//! emissive orb. Because they are tall and glow (via the camera's HDR + bloom)
//! they read as "lighthouses" from across the map, pulling the player toward
//! them — the classic open-world curiosity/leading-anchor design. Walking
//! close to one collects it: it vanishes and the objective counter ticks up.
//!
//! The whole feature is data-light: one shared pillar mesh + orb mesh + orb
//! material cloned across every beacon, a seeded RNG for deterministic
//! placement on hilltops, and two small systems (animate, collect).

use crate::player::Collider;
use crate::states::GameState;
use crate::terrain::{self, HALF_SIZE};
use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

/// How many beacons to place in the world (the long-term goal).
const BEACON_COUNT: usize = 12;
/// Player must get within this horizontal distance to collect a beacon.
const COLLECT_RADIUS: f32 = 4.0;
/// Beacons only spawn above this elevation, so they crown hilltops/vistas.
const MIN_ELEVATION: f32 = 6.0;
/// Keep beacons spread out so each is a distinct destination.
const MIN_SEPARATION: f32 = 45.0;
const ORB_HEIGHT: f32 = 7.0;

pub struct BeaconsPlugin;

impl Plugin for BeaconsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Score {
            collected: 0,
            total: 0,
        })
        .add_message::<BeaconCollected>()
        .add_systems(Startup, spawn_beacons)
        .add_systems(
            Update,
            (animate_orbs, collect_beacons).run_if(in_state(GameState::Playing)),
        );
    }
}

/// Objective progress. `total` is filled in at spawn time.
#[derive(Resource)]
pub struct Score {
    pub collected: u32,
    pub total: u32,
}

/// Fired when the player collects a beacon; the HUD listens for feedback.
#[derive(Message)]
pub struct BeaconCollected;

/// A collectible beacon. The glowing orb is a child entity so despawning the
/// beacon (via `despawn` recursion) removes both parts.
#[derive(Component)]
struct Beacon;

/// The floating orb child, animated by `animate_orbs`.
#[derive(Component)]
struct Orb {
    /// Per-orb phase offset so they don't all bob in lockstep.
    phase: f32,
}

fn spawn_beacons(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut score: ResMut<Score>,
) {
    let pillar_mesh = meshes.add(Cuboid::new(0.6, ORB_HEIGHT, 0.6));
    let orb_mesh = meshes.add(Sphere::new(0.8));

    let pillar_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.08, 0.10),
        perceptual_roughness: 0.4,
        ..default()
    });
    // Emissive well above 1.0 so HDR + bloom make it glow.
    let orb_mat = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(0.2, 5.0, 7.0),
        ..default()
    });

    let positions = pick_hilltops();
    score.total = positions.len() as u32;

    for base in positions {
        commands
            .spawn((
                Beacon,
                Transform::from_translation(base),
                Visibility::default(),
                Mesh3d(pillar_mesh.clone()),
                MeshMaterial3d(pillar_mat.clone()),
                // Solid pillar you must walk around; despawning on collect
                // removes the collider with it.
                Collider { radius: 0.6 },
            ))
            .with_children(|beacon| {
                // Orb floats above the pillar top; child transform is local.
                beacon.spawn((
                    Orb {
                        phase: base.x + base.z,
                    },
                    Mesh3d(orb_mesh.clone()),
                    MeshMaterial3d(orb_mat.clone()),
                    Transform::from_xyz(0.0, ORB_HEIGHT * 0.5 + 1.5, 0.0),
                    // A soft cyan point light makes the orb spill glow onto the
                    // nearby ground, selling it as a real light source.
                    PointLight {
                        color: Color::srgb(0.4, 0.9, 1.0),
                        intensity: 250_000.0,
                        range: 30.0,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                ));
            });
    }
}

/// Deterministically choose spread-out hilltop positions for the beacons.
fn pick_hilltops() -> Vec<Vec3> {
    let mut rng = StdRng::seed_from_u64(0xB3AC0Au64);
    let reach = HALF_SIZE - 10.0;
    let mut chosen: Vec<Vec3> = Vec::with_capacity(BEACON_COUNT);

    // Rejection sampling: keep candidates that are high, fairly flat, and far
    // enough from already-placed beacons. Bounded attempts so it always ends.
    for _ in 0..4000 {
        if chosen.len() == BEACON_COUNT {
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

/// Collect any beacon the player has walked close to (horizontal distance
/// only, so you don't need to be at the orb's exact height). Emits an event
/// for HUD feedback and updates the score.
fn collect_beacons(
    mut commands: Commands,
    mut score: ResMut<Score>,
    mut collected: MessageWriter<BeaconCollected>,
    player: Single<&Transform, With<Camera3d>>,
    beacons: Query<(Entity, &Transform), With<Beacon>>,
) {
    let p = player.translation;
    for (entity, tf) in &beacons {
        let d = Vec2::new(tf.translation.x - p.x, tf.translation.z - p.z).length();
        if d < COLLECT_RADIUS {
            commands.entity(entity).despawn();
            score.collected += 1;
            collected.write(BeaconCollected);
        }
    }
}
