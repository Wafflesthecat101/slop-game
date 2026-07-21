//! The open world: terrain mesh, sky/lighting, and scattered scenery.
//!
//! Everything here is built once at [`Startup`]. The terrain is a single
//! textured mesh generated from [`crate::terrain::height`]; trees and rocks
//! are scattered deterministically on top of it. Shared cube/cylinder meshes
//! and materials are cloned by handle, so thousands of objects cost only a
//! handful of GPU resources.

use crate::player::Collider;
use crate::terrain::{self, HALF_SIZE};
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

/// Number of quads per side of the terrain grid. 200 -> 40k quads, plenty of
/// resolution for 400x400m of hills while staying light on the GPU.
const GRID: usize = 200;

/// How many times ground textures repeat across the whole terrain.
const GROUND_TILING: f32 = 60.0;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        // Sky/horizon colour, reused for the distance fog on the camera so the
        // world dissolves seamlessly into the sky at the far plane.
        app.insert_resource(ClearColor(SKY_COLOR))
            .insert_resource(GlobalAmbientLight {
                color: Color::srgb(0.75, 0.8, 0.95),
                brightness: 280.0,
                ..default()
            })
            .add_systems(
                Startup,
                (setup_sky_and_light, setup_terrain, scatter_scenery),
            );
    }
}

/// Warm late-afternoon sky colour, shared by the clear colour and the fog.
pub const SKY_COLOR: Color = Color::srgb(0.70, 0.80, 0.92);

/// A textured [`StandardMaterial`] that tiles its base-color image `tiling`
/// times across the terrain UV range.
///
/// The image is loaded with a **repeating** sampler: Bevy's default sampler
/// address mode is `ClampToEdge`, which would clamp our tiled UVs (0..N) to
/// the texture's edge texel and make the whole surface a single flat colour.
/// `Repeat` is what actually makes the texture tile across the terrain.
fn textured(
    assets: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
    path: &'static str,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color_texture: Some(
            assets
                .load_builder()
                .with_settings(repeating_sampler)
                .load(path),
        ),
        perceptual_roughness: 0.95,
        ..default()
    })
}

/// Image loader settings that make a texture tile (repeat) instead of clamping
/// to its edge (Bevy's default), with linear min/mag filtering.
fn repeating_sampler(settings: &mut ImageLoaderSettings) {
    settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
}

/// Like [`textured`] for the leaves image but multiplied by `tint`, so several
/// canopy colours can share the single leaves texture.
fn tinted_leaves(
    assets: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
    tint: Color,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: tint,
        base_color_texture: Some(assets.load("textures/leaves.png")),
        perceptual_roughness: 0.95,
        ..default()
    })
}

fn setup_sky_and_light(mut commands: Commands) {
    // A single warm, low directional "sun" (golden-hour angle) with shadows —
    // low sun angle gives long, readable shadows and a warmer, moodier scene
    // than a flat overhead light. `directional_light_color` on the fog below
    // then tints the haze toward the sun for atmospheric depth.
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.93, 0.78),
            illuminance: 11_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(80.0, 55.0, 30.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn setup_terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
) {
    commands.spawn((
        Mesh3d(meshes.add(build_terrain_mesh())),
        MeshMaterial3d(textured(&assets, &mut materials, "textures/grass.png")),
        Transform::default(),
    ));
}

/// Builds the terrain mesh: a `GRID x GRID` grid of vertices displaced by
/// [`terrain::height`], with per-vertex normals and tiled UVs.
fn build_terrain_mesh() -> Mesh {
    let verts_per_side = GRID + 1;
    let step = (HALF_SIZE * 2.0) / GRID as f32;

    let mut positions = Vec::with_capacity(verts_per_side * verts_per_side);
    let mut normals = Vec::with_capacity(verts_per_side * verts_per_side);
    let mut uvs = Vec::with_capacity(verts_per_side * verts_per_side);
    let mut colors = Vec::with_capacity(verts_per_side * verts_per_side);

    for iz in 0..verts_per_side {
        for ix in 0..verts_per_side {
            let x = -HALF_SIZE + ix as f32 * step;
            let z = -HALF_SIZE + iz as f32 * step;
            let y = terrain::height(x, z);
            positions.push([x, y, z]);
            normals.push(terrain::normal(x, z).to_array());
            let u = ix as f32 / GRID as f32 * GROUND_TILING;
            let v = iz as f32 / GRID as f32 * GROUND_TILING;
            uvs.push([u, v]);
            // Per-vertex biome tint (multiplies the grass texture) gives the
            // single mesh sandy lowlands, green midlands and rocky highlands.
            let [r, g, b] = terrain::biome_tint(y);
            colors.push([r, g, b, 1.0]);
        }
    }

    let mut indices = Vec::with_capacity(GRID * GRID * 6);
    for iz in 0..GRID {
        for ix in 0..GRID {
            let tl = (iz * verts_per_side + ix) as u32;
            let tr = tl + 1;
            let bl = tl + verts_per_side as u32;
            let br = bl + 1;
            indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
        }
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

/// Scatters trees (trunk + leaf canopy) and rocks across the terrain. Uses a
/// seeded RNG so the world is identical every run, and shares one mesh +
/// material per object kind so the whole forest is cheap to render.
fn scatter_scenery(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
) {
    let trunk_mesh = meshes.add(Cylinder::new(0.35, 1.0));
    let canopy_mesh = meshes.add(Sphere::new(1.0));
    let rock_mesh = meshes.add(Sphere::new(1.0));

    let bark = textured(&assets, &mut materials, "textures/bark.png");
    let rock = textured(&assets, &mut materials, "textures/rock.png");

    // A small palette of leaf tints (multiplying the one leaves texture) gives
    // the forest colour variety — deep green, olive, and autumnal — while
    // still being just three shared materials.
    let leaf_palette: [Handle<StandardMaterial>; 3] = [
        tinted_leaves(&assets, &mut materials, Color::srgb(0.55, 0.75, 0.45)),
        tinted_leaves(&assets, &mut materials, Color::srgb(0.70, 0.72, 0.38)),
        tinted_leaves(&assets, &mut materials, Color::srgb(0.85, 0.62, 0.35)),
    ];

    let mut rng = StdRng::seed_from_u64(20240720);
    let placeable = HALF_SIZE - 5.0;

    for _ in 0..600 {
        let x = rng.random_range(-placeable..placeable);
        let z = rng.random_range(-placeable..placeable);
        let y = terrain::height(x, z);

        // Skip steep slopes so nothing floats off a cliff face.
        if terrain::normal(x, z).y < 0.85 {
            continue;
        }

        if rng.random_bool(0.8) {
            let leaves = &leaf_palette[rng.random_range(0..leaf_palette.len())];
            spawn_tree(
                &mut commands,
                &trunk_mesh,
                &canopy_mesh,
                &bark,
                leaves,
                &mut rng,
                Vec3::new(x, y, z),
            );
        } else {
            let scale = rng.random_range(0.6..2.2);
            commands.spawn((
                Mesh3d(rock_mesh.clone()),
                MeshMaterial3d(rock.clone()),
                Transform::from_translation(Vec3::new(x, y + scale * 0.4, z))
                    .with_scale(Vec3::splat(scale)),
                // Rock mesh is a unit sphere, so its world radius is `scale`;
                // 0.7 keeps the walkable edge close to the visible surface.
                Collider {
                    radius: scale * 0.7,
                },
            ));
        }
    }
}

fn spawn_tree(
    commands: &mut Commands,
    trunk_mesh: &Handle<Mesh>,
    canopy_mesh: &Handle<Mesh>,
    bark: &Handle<StandardMaterial>,
    leaves: &Handle<StandardMaterial>,
    rng: &mut StdRng,
    base: Vec3,
) {
    let trunk_h = rng.random_range(3.0..6.0);
    let canopy_r = rng.random_range(1.8..3.2);

    commands.spawn((
        Mesh3d(trunk_mesh.clone()),
        MeshMaterial3d(bark.clone()),
        Transform::from_translation(base + Vec3::Y * trunk_h * 0.5)
            .with_scale(Vec3::new(1.0, trunk_h, 1.0)),
        // Block the trunk footprint; the overhead canopy needs no collider.
        Collider { radius: 0.6 },
    ));
    commands.spawn((
        Mesh3d(canopy_mesh.clone()),
        MeshMaterial3d(leaves.clone()),
        Transform::from_translation(base + Vec3::Y * (trunk_h + canopy_r * 0.6))
            .with_scale(Vec3::splat(canopy_r)),
    ));
}
