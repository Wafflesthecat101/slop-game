//! Points of interest — deterministic landmarks that reward wandering.
//!
//! Scattered across the world are a handful of memorable, hand-shaped
//! landmarks — a ring of standing stones, a lone great tree, a cliff-edge
//! overlook cairn — that give the eye destinations beyond the shrines and
//! frame the vistas the terrain already builds (design pillar P3).
//!
//! Like the scenery in [`crate::world`] and the shrines in [`crate::beacons`],
//! this stays cheap by construction (P4): one shared mesh/material per part,
//! cloned across every placement, and a fixed-seed [`StdRng`] so the same
//! landmarks appear in the same spots every run. Placement is pure rejection
//! sampling over [`terrain::height`]/[`terrain::normal`] that skips steep
//! slopes and the high, flat hilltops the shrines claim, so POIs never land on
//! a cliff face or on top of a shrine.
//!
//! No new lights: the overlook cairn's tip uses an emissive material so it
//! catches the eye at distance through the camera's existing bloom, without
//! adding a `PointLight` (which would perturb the world's light budget).

use crate::player::Collider;
use crate::terrain::{self, HALF_SIZE};
use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

/// How many landmarks to scatter. Kept small so each is a rare, memorable find.
const POI_COUNT: usize = 6;
/// Keep landmarks spread out so each is its own destination.
const MIN_SEPARATION: f32 = 60.0;
/// Reject slopes steeper than this (matches the scenery scatter in `world.rs`).
const SLOPE_MIN: f32 = 0.85;
/// A cell counts as a shrine-style hilltop (and is avoided) when it is at least
/// this high and this flat — the same thresholds `beacons::pick_hilltops` uses,
/// so POIs deterministically steer clear of where shrines spawn.
const SHRINE_ELEVATION: f32 = 6.0;
const SHRINE_FLATNESS: f32 = 0.9;

/// The distinct kinds of landmark. Each reads differently from afar.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum PoiKind {
    /// A ring of standing stones around an open centre.
    StoneRing,
    /// A single, oversized tree that towers over the surrounding forest.
    GreatTree,
    /// A flat stone ledge topped by a small glowing cairn — a place to look out.
    CliffOverlook,
}

impl PoiKind {
    /// Cycle through the kinds by placement index so every kind appears and the
    /// mix is deterministic.
    fn from_index(i: usize) -> Self {
        match i % 3 {
            0 => PoiKind::StoneRing,
            1 => PoiKind::GreatTree,
            _ => PoiKind::CliffOverlook,
        }
    }
}

pub struct PoiPlugin;

impl Plugin for PoiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_pois);
    }
}

fn spawn_pois(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
) {
    // One shared mesh per part; a unit sphere/cylinder scaled per placement.
    let stone_mesh = meshes.add(Sphere::new(1.0));
    let ledge_mesh = meshes.add(Cuboid::new(6.0, 0.6, 6.0));
    let trunk_mesh = meshes.add(Cylinder::new(0.6, 1.0));
    let canopy_mesh = meshes.add(Sphere::new(1.0));

    let stone_mat = materials.add(StandardMaterial {
        base_color_texture: Some(assets.load("textures/rock.png")),
        perceptual_roughness: 0.95,
        ..default()
    });
    let bark_mat = materials.add(StandardMaterial {
        base_color_texture: Some(assets.load("textures/bark.png")),
        perceptual_roughness: 0.95,
        ..default()
    });
    let leaf_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.58, 0.32),
        base_color_texture: Some(assets.load("textures/leaves.png")),
        perceptual_roughness: 0.95,
        ..default()
    });
    // Warm emissive so the overlook cairn glows through the camera's bloom and
    // pulls the eye toward the vista — no PointLight needed.
    let cairn_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.15, 0.1),
        emissive: LinearRgba::rgb(4.0, 2.6, 1.0),
        ..default()
    });

    for (i, base) in pick_pois().into_iter().enumerate() {
        match PoiKind::from_index(i) {
            PoiKind::StoneRing => spawn_stone_ring(&mut commands, &stone_mesh, &stone_mat, base),
            PoiKind::GreatTree => spawn_great_tree(
                &mut commands,
                &trunk_mesh,
                &canopy_mesh,
                &bark_mat,
                &leaf_mat,
                base,
            ),
            PoiKind::CliffOverlook => spawn_overlook(
                &mut commands,
                &ledge_mesh,
                &stone_mesh,
                &stone_mat,
                &cairn_mat,
                base,
            ),
        }
    }
}

/// Eight standing stones evenly spaced around `base`, each following the ground.
fn spawn_stone_ring(
    commands: &mut Commands,
    stone_mesh: &Handle<Mesh>,
    stone_mat: &Handle<StandardMaterial>,
    base: Vec3,
) {
    const STONES: usize = 8;
    const RING_RADIUS: f32 = 5.0;
    for k in 0..STONES {
        let angle = k as f32 / STONES as f32 * std::f32::consts::TAU;
        let x = base.x + angle.cos() * RING_RADIUS;
        let z = base.z + angle.sin() * RING_RADIUS;
        let y = terrain::height(x, z);
        // Tall, narrow menhirs: a unit sphere squashed on X/Z and stretched on Y.
        let scale = Vec3::new(0.8, 2.6, 0.8);
        commands.spawn((
            Mesh3d(stone_mesh.clone()),
            MeshMaterial3d(stone_mat.clone()),
            Transform::from_translation(Vec3::new(x, y + scale.y * 0.5, z)).with_scale(scale),
            Collider { radius: 0.8 },
        ));
    }
}

/// A single oversized tree — a big trunk with a broad canopy.
fn spawn_great_tree(
    commands: &mut Commands,
    trunk_mesh: &Handle<Mesh>,
    canopy_mesh: &Handle<Mesh>,
    bark_mat: &Handle<StandardMaterial>,
    leaf_mat: &Handle<StandardMaterial>,
    base: Vec3,
) {
    let trunk_h = 12.0;
    let canopy_r = 7.0;
    commands.spawn((
        Mesh3d(trunk_mesh.clone()),
        MeshMaterial3d(bark_mat.clone()),
        Transform::from_translation(base + Vec3::Y * trunk_h * 0.5)
            .with_scale(Vec3::new(1.0, trunk_h, 1.0)),
        Collider { radius: 1.0 },
    ));
    commands.spawn((
        Mesh3d(canopy_mesh.clone()),
        MeshMaterial3d(leaf_mat.clone()),
        Transform::from_translation(base + Vec3::Y * (trunk_h + canopy_r * 0.5))
            .with_scale(Vec3::splat(canopy_r)),
    ));
}

/// A flat stone ledge with a small glowing cairn on it — a framed lookout.
fn spawn_overlook(
    commands: &mut Commands,
    ledge_mesh: &Handle<Mesh>,
    stone_mesh: &Handle<Mesh>,
    stone_mat: &Handle<StandardMaterial>,
    cairn_mat: &Handle<StandardMaterial>,
    base: Vec3,
) {
    commands.spawn((
        Mesh3d(ledge_mesh.clone()),
        MeshMaterial3d(stone_mat.clone()),
        Transform::from_translation(base + Vec3::Y * 0.3),
    ));
    // A stacked cairn: three shrinking stones, the tip emissive so it glints.
    let stack = [(1.0, 0.9), (0.9, 2.2), (0.6, 3.2)];
    for (i, (r, y)) in stack.iter().enumerate() {
        let mat = if i == stack.len() - 1 {
            cairn_mat.clone()
        } else {
            stone_mat.clone()
        };
        commands.spawn((
            Mesh3d(stone_mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(base + Vec3::new(0.0, *y, 0.0)).with_scale(Vec3::splat(*r)),
        ));
    }
}

/// Whether a cell is a suitable landmark site: not a steep slope, and not one
/// of the high, flat hilltops the shrines claim (so POIs never overlap them).
fn is_suitable(x: f32, z: f32) -> bool {
    let n = terrain::normal(x, z);
    if n.y < SLOPE_MIN {
        return false;
    }
    let y = terrain::height(x, z);
    !(y >= SHRINE_ELEVATION && n.y >= SHRINE_FLATNESS)
}

/// Deterministically choose spread-out landmark positions via rejection
/// sampling over suitable terrain. Identical every run (fixed seed).
fn pick_pois() -> Vec<Vec3> {
    let mut rng = StdRng::seed_from_u64(0x9E17A1u64);
    let reach = HALF_SIZE - 15.0;
    let mut chosen: Vec<Vec3> = Vec::with_capacity(POI_COUNT);

    for _ in 0..6000 {
        if chosen.len() == POI_COUNT {
            break;
        }
        let x = rng.random_range(-reach..reach);
        let z = rng.random_range(-reach..reach);
        if !is_suitable(x, z) {
            continue;
        }
        let pos = Vec3::new(x, terrain::height(x, z), z);
        if chosen.iter().any(|p| p.distance(pos) < MIN_SEPARATION) {
            continue;
        }
        chosen.push(pos);
    }
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_is_deterministic() {
        assert_eq!(pick_pois(), pick_pois());
    }

    #[test]
    fn places_the_full_set() {
        assert_eq!(pick_pois().len(), POI_COUNT);
    }

    #[test]
    fn covers_at_least_three_distinct_kinds() {
        let kinds: std::collections::HashSet<_> =
            (0..pick_pois().len()).map(PoiKind::from_index).collect();
        assert!(
            kinds.len() >= 3,
            "expected >=3 POI kinds, got {}",
            kinds.len()
        );
    }

    #[test]
    fn all_sites_are_suitable_and_separated() {
        let pois = pick_pois();
        for p in &pois {
            assert!(
                is_suitable(p.x, p.z),
                "POI at ({}, {}) landed on unsuitable terrain",
                p.x,
                p.z
            );
        }
        for (i, a) in pois.iter().enumerate() {
            for b in &pois[i + 1..] {
                assert!(
                    a.distance(*b) >= MIN_SEPARATION,
                    "POIs too close: {a:?} vs {b:?}"
                );
            }
        }
    }

    #[test]
    fn rejects_steep_slopes_and_shrine_hilltops() {
        // A high, flat cell (shrine territory) must be rejected even though it
        // is not steep, so POIs and shrines never share a spot.
        let mut checked_shrine_like = false;
        let mut x = -HALF_SIZE;
        while x <= HALF_SIZE {
            let mut z = -HALF_SIZE;
            while z <= HALF_SIZE {
                let n = terrain::normal(x, z);
                if terrain::height(x, z) >= SHRINE_ELEVATION && n.y >= SHRINE_FLATNESS {
                    assert!(!is_suitable(x, z));
                    checked_shrine_like = true;
                }
                if n.y < SLOPE_MIN {
                    assert!(!is_suitable(x, z));
                }
                z += 5.0;
            }
            x += 5.0;
        }
        assert!(checked_shrine_like, "no shrine-like cell sampled");
    }
}
