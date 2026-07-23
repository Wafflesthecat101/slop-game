//! Ambient birds — a boids-style flock drifting over the hills.
//!
//! Flocks of birds make the sky feel alive as the world reawakens (pillar P3).
//! The flock is deliberately cheap (P4): a fixed, capped number of birds all
//! share **one** mesh + **one** unlit material, so the whole flock is a handful
//! of GPU resources, and the per-frame simulation reuses scratch buffers held
//! in a `Local` — no per-bird heap allocation each frame.
//!
//! The flocking itself is the classic Reynolds boids model — three steering
//! rules blended together:
//! * **separation** — steer away from neighbours that are too close,
//! * **alignment** — match the average heading of nearby birds,
//! * **cohesion** — steer toward the average position (centroid) of neighbours.
//!
//! The blend lives in a pure, allocation-free helper ([`steer`]) that takes
//! flat position/velocity slices and returns one bird's new velocity, so the
//! flocking logic is unit-tested without spinning up a Bevy `App`. The ECS
//! system just snapshots the flock into scratch buffers, calls [`steer`] per
//! bird, then writes the results back, keeping birds above the terrain and
//! wrapped within the world bounds.
//!
//! Flock size scales *gently* with world [`Progress`]: a core group is always
//! present, and more birds fade in as shrines are rekindled.

use crate::beacons::Progress;
use crate::states::GameState;
use crate::terrain::{self, HALF_SIZE};
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

/// Hard cap on the flock (P4). Small enough to stay cheap, large enough to read
/// as a proper flock in the sky.
const MAX_BIRDS: usize = 32;
/// Birds always present regardless of progress (the world is never empty).
const MIN_BIRDS: usize = 12;

/// Birds roam within `±ROAM_BOUND` on X/Z and wrap to the far side when they
/// cross it, so the flock never leaves the map.
const ROAM_BOUND: f32 = HALF_SIZE - 10.0;
/// Preferred cruising height above the ground the flock eases toward.
const CRUISE_HEIGHT: f32 = 38.0;
/// Birds are never allowed below this clearance above the terrain.
const MIN_CLEARANCE: f32 = 22.0;
/// How strongly birds ease back toward their cruising altitude (per second).
const ALTITUDE_WEIGHT: f32 = 0.6;

/// Deterministic seed so the initial flock is identical every run (matches the
/// fixed-seed scenery/shrine placement elsewhere).
const FLOCK_SEED: u64 = 0xB1D5_F10C;

/// Steering weights + neighbourhood radii for the boids blend. Kept as data so
/// the pure [`steer`] helper is easy to exercise rule-by-rule in tests.
#[derive(Clone, Copy)]
pub struct BoidParams {
    /// Neighbours within this distance contribute to alignment + cohesion.
    pub neighbor_radius: f32,
    /// Neighbours closer than this trigger separation (push-away).
    pub separation_radius: f32,
    pub separation_weight: f32,
    pub alignment_weight: f32,
    pub cohesion_weight: f32,
    /// Speed band the flock is clamped into so birds keep gliding but never race.
    pub min_speed: f32,
    pub max_speed: f32,
}

impl Default for BoidParams {
    fn default() -> Self {
        Self {
            neighbor_radius: 22.0,
            separation_radius: 7.0,
            separation_weight: 4.0,
            alignment_weight: 2.0,
            cohesion_weight: 0.8,
            min_speed: 6.0,
            max_speed: 14.0,
        }
    }
}

/// A single bird. `velocity` is the flock-simulation state; `appear_at` is the
/// [`Progress::fraction`] at which this bird fades into the flock (`0.0` for the
/// always-present core group).
#[derive(Component)]
pub struct Bird {
    pub velocity: Vec3,
    pub appear_at: f32,
}

/// Reused scratch buffers for the flock step, held in a `Local` so the hot path
/// allocates nothing per frame after warm-up (P4).
#[derive(Default)]
struct FlockScratch {
    entities: Vec<Entity>,
    positions: Vec<Vec3>,
    velocities: Vec<Vec3>,
    new_velocities: Vec<Vec3>,
}

pub struct BirdsPlugin;

impl Plugin for BirdsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_flock).add_systems(
            Update,
            (flock, scale_flock_with_progress).run_if(in_state(GameState::Playing)),
        );
    }
}

/// One bird's new velocity from the boids blend, given the whole flock as flat
/// slices. Pure and allocation-free so it can be unit-tested in isolation.
///
/// `positions[index]` / `velocities[index]` are this bird's current state;
/// `dt` scales the steering so behaviour is frame-rate independent. The result
/// is clamped into `[min_speed, max_speed]`.
pub fn steer(
    index: usize,
    positions: &[Vec3],
    velocities: &[Vec3],
    params: &BoidParams,
    dt: f32,
) -> Vec3 {
    let pos = positions[index];
    let vel = velocities[index];

    let mut separation = Vec3::ZERO;
    let mut heading_sum = Vec3::ZERO;
    let mut center_sum = Vec3::ZERO;
    let mut neighbors = 0u32;

    for (i, (&other_pos, &other_vel)) in positions.iter().zip(velocities.iter()).enumerate() {
        if i == index {
            continue;
        }
        let offset = pos - other_pos;
        let dist = offset.length();
        if dist < params.neighbor_radius {
            heading_sum += other_vel;
            center_sum += other_pos;
            neighbors += 1;
            if dist > 0.0 && dist < params.separation_radius {
                // Push away, harder the closer the neighbour is.
                separation += offset / dist * (params.separation_radius - dist);
            }
        }
    }

    let mut acceleration = separation * params.separation_weight;
    if neighbors > 0 {
        let inv = 1.0 / neighbors as f32;
        let avg_heading = heading_sum * inv;
        let centroid = center_sum * inv;
        acceleration += (avg_heading - vel) * params.alignment_weight;
        acceleration += (centroid - pos) * params.cohesion_weight;
    }

    let mut new_vel = vel + acceleration * dt;
    let speed = new_vel.length();
    if speed > params.max_speed {
        new_vel = new_vel / speed * params.max_speed;
    } else if speed > 1e-5 && speed < params.min_speed {
        new_vel = new_vel / speed * params.min_speed;
    }
    new_vel
}

/// Spawns the full capped flock at startup, sharing one mesh + one unlit
/// material. Placement/velocity use a fixed-seed RNG so the flock is identical
/// every run. Birds beyond the core group get a staggered `appear_at` so they
/// fade in as the world reawakens.
fn spawn_flock(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(bird_mesh());
    // Unlit, faintly emissive dark silhouette — no lights added (keeps the CI
    // light-count invariants intact), still reads against a bright sky.
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.06, 0.07, 0.10),
        emissive: LinearRgba::rgb(0.03, 0.03, 0.05),
        unlit: true,
        ..default()
    });

    let mut rng = StdRng::seed_from_u64(FLOCK_SEED);
    let scatter = ROAM_BOUND * 0.4;

    for i in 0..MAX_BIRDS {
        let x = rng.random_range(-scatter..scatter);
        let z = rng.random_range(-scatter..scatter);
        let y = terrain::height(x, z) + CRUISE_HEIGHT + rng.random_range(-4.0..4.0);

        // Random heading in the XZ plane at a comfortable cruise speed.
        let angle = rng.random_range(0.0..std::f32::consts::TAU);
        let speed = rng.random_range(7.0..12.0);
        let velocity = Vec3::new(angle.cos() * speed, 0.0, angle.sin() * speed);

        // First MIN_BIRDS are always visible; the rest fade in with progress.
        let appear_at = if i < MIN_BIRDS {
            0.0
        } else {
            (i - MIN_BIRDS) as f32 / (MAX_BIRDS - MIN_BIRDS) as f32
        };

        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(Vec3::new(x, y, z)),
            Bird {
                velocity,
                appear_at,
            },
        ));
    }
}

/// Advances the whole flock one step: snapshot into scratch buffers, run the
/// pure boids [`steer`] per bird, then write velocities/positions back while
/// keeping each bird above the terrain and wrapped within the roam bounds.
fn flock(
    time: Res<Time>,
    params: Local<BoidParams>,
    mut scratch: Local<FlockScratch>,
    mut birds: Query<(Entity, &mut Transform, &mut Bird)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    scratch.entities.clear();
    scratch.positions.clear();
    scratch.velocities.clear();
    for (entity, transform, bird) in &birds {
        scratch.entities.push(entity);
        scratch.positions.push(transform.translation);
        scratch.velocities.push(bird.velocity);
    }

    scratch.new_velocities.clear();
    for i in 0..scratch.positions.len() {
        let v = steer(i, &scratch.positions, &scratch.velocities, &params, dt);
        scratch.new_velocities.push(v);
    }

    // Apply by stored entity (via `get_mut`) so we never rely on query
    // iteration order matching between the snapshot and apply passes.
    for i in 0..scratch.entities.len() {
        let Ok((_, mut transform, mut bird)) = birds.get_mut(scratch.entities[i]) else {
            continue;
        };

        let mut vel = scratch.new_velocities[i];
        let mut pos = scratch.positions[i];

        // Ease back toward the preferred cruising altitude above the ground.
        let desired_y = terrain::height(pos.x, pos.z) + CRUISE_HEIGHT;
        vel.y += (desired_y - pos.y) * ALTITUDE_WEIGHT * dt;

        pos += vel * dt;
        pos.x = wrap(pos.x, ROAM_BOUND);
        pos.z = wrap(pos.z, ROAM_BOUND);

        // Hard floor: never let a bird dip into the hills.
        let floor = terrain::height(pos.x, pos.z) + MIN_CLEARANCE;
        if pos.y < floor {
            pos.y = floor;
            vel.y = vel.y.max(0.0);
        }

        transform.translation = pos;
        if vel.length_squared() > 1e-4 {
            transform.look_to(vel.normalize(), Vec3::Y);
        }
        bird.velocity = vel;
    }
}

/// Fades birds beyond the core group in/out based on world [`Progress`], so the
/// flock grows *gently* as shrines are rekindled. Read-only on `Progress`.
fn scale_flock_with_progress(progress: Res<Progress>, mut birds: Query<(&Bird, &mut Visibility)>) {
    let fraction = progress.fraction();
    for (bird, mut visibility) in &mut birds {
        let wanted = if fraction >= bird.appear_at {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// Wraps a coordinate into `[-bound, bound]` by teleporting across to the far
/// edge, so the flock roams the whole map without ever leaving it.
fn wrap(v: f32, bound: f32) -> f32 {
    if v > bound {
        v - 2.0 * bound
    } else if v < -bound {
        v + 2.0 * bound
    } else {
        v
    }
}

/// A tiny two-winged dart silhouette pointing along `-Z` (Bevy's forward axis),
/// so orienting a bird to its velocity aims the nose into the wind. Two
/// oppositely-wound triangles make it visible from both sides without needing a
/// custom cull mode.
fn bird_mesh() -> Mesh {
    let positions = vec![
        [0.0, 0.0, -0.7], // nose
        [-0.6, 0.0, 0.5], // left wing
        [0.6, 0.0, 0.5],  // right wing
    ];
    let normals = vec![[0.0, 1.0, 0.0]; 3];
    let uvs = vec![[0.5, 0.0], [0.0, 1.0], [1.0, 1.0]];
    let indices = Indices::U32(vec![0, 1, 2, 0, 2, 1]);

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params_only(separation: f32, alignment: f32, cohesion: f32) -> BoidParams {
        BoidParams {
            neighbor_radius: 100.0,
            separation_radius: 10.0,
            separation_weight: separation,
            alignment_weight: alignment,
            cohesion_weight: cohesion,
            // Wide band so clamping never masks the steering direction we assert.
            min_speed: 0.0,
            max_speed: 1e6,
        }
    }

    #[test]
    fn separation_pushes_close_birds_apart() {
        // Two birds nearly on top of each other; only separation active.
        let positions = [Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0)];
        let velocities = [Vec3::ZERO, Vec3::ZERO];
        let params = params_only(1.0, 0.0, 0.0);

        let v0 = steer(0, &positions, &velocities, &params, 1.0);
        let v1 = steer(1, &positions, &velocities, &params, 1.0);

        // Bird 0 (at origin) is pushed toward -X, away from bird 1; vice versa.
        assert!(v0.x < 0.0, "bird 0 should be pushed away (-X), got {v0:?}");
        assert!(v1.x > 0.0, "bird 1 should be pushed away (+X), got {v1:?}");
    }

    #[test]
    fn cohesion_pulls_toward_centroid() {
        // Three birds; bird 0 sits off to the side of the other two, beyond the
        // separation radius so only cohesion acts.
        let positions = [
            Vec3::new(-30.0, 0.0, 0.0),
            Vec3::new(20.0, 0.0, 0.0),
            Vec3::new(20.0, 0.0, 10.0),
        ];
        let velocities = [Vec3::ZERO; 3];
        let params = params_only(0.0, 0.0, 1.0);

        let v0 = steer(0, &positions, &velocities, &params, 1.0);

        // Centroid of the neighbours is at +X (and +Z), so bird 0 accelerates
        // toward it.
        assert!(
            v0.x > 0.0,
            "cohesion should pull toward centroid (+X), got {v0:?}"
        );
    }

    #[test]
    fn alignment_matches_neighbor_heading() {
        // Bird 0 is still; its neighbour cruises along +X. Only alignment active.
        let positions = [Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0)];
        let velocities = [Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)];
        let params = params_only(0.0, 1.0, 0.0);

        let v0 = steer(0, &positions, &velocities, &params, 1.0);

        // Bird 0 gains velocity along the neighbour's heading.
        assert!(
            v0.x > 0.0,
            "alignment should match neighbour heading (+X), got {v0:?}"
        );
        assert!(
            v0.z.abs() < 1e-4,
            "no cross-axis drift expected, got {v0:?}"
        );
    }

    #[test]
    fn speed_is_clamped_into_band() {
        let positions = [Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0)];
        let velocities = [Vec3::new(1000.0, 0.0, 0.0), Vec3::ZERO];
        let params = BoidParams {
            min_speed: 6.0,
            max_speed: 14.0,
            ..params_only(0.0, 0.0, 0.0)
        };

        let v = steer(0, &positions, &velocities, &params, 1.0);
        assert!(
            v.length() <= params.max_speed + 1e-3,
            "speed exceeded max: {}",
            v.length()
        );
        assert!(
            v.length() >= params.min_speed - 1e-3,
            "speed below min: {}",
            v.length()
        );
    }

    #[test]
    fn wrap_keeps_within_bounds() {
        assert_eq!(wrap(0.0, 10.0), 0.0);
        assert_eq!(wrap(11.0, 10.0), -9.0);
        assert_eq!(wrap(-11.0, 10.0), 9.0);
    }
}
