//! Pool water — refraction, caustics and a rippling height simulation, as a
//! native plugin.
//!
//! In the workspace this was two crates, `renzora_pool_water` (Runtime) and
//! `renzora_pool_water_editor` (Editor), split so a shipped game would not drag
//! in the editor framework. Here they are one `Runtime` plugin: it loads in the
//! editor *and* in a copy-based export, and the inspector registration costs a
//! game one `Vec` push into a registry it never reads.
//!
//! Two things had to change coming across, and both are silent failures rather
//! than compile errors if missed:
//!
//! * **`serde` comes from `renzora`.** A plugin's own crates.io dependencies are
//!   resolved separately from the engine's, so a plain `serde = "1"` is a
//!   different crate from the one Bevy derives against. Using the contract
//!   crate's copy also means cargo never runs for this plugin at all.
//! * **No `cfg(feature = "editor")`.** A native plugin is compiled by `rustc`
//!   with no features set, so that gate is always false — the inspector would
//!   vanish with nothing logged.

pub mod inspector;
pub mod material;
pub mod simulation;

use bevy::asset::embedded_asset;
use bevy::asset::RenderAssetUsages;
use bevy::core_pipeline::prepass::DepthPrepass;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::pbr::MaterialPlugin;
use bevy::prelude::*;
use renzora::serde::{Deserialize, Serialize};

use material::{PoolWaterMaterial, PoolWaterUniforms};
use simulation::WaterSim;


// ── Components ────────────────────────────────────────────────────────────────

/// Marker on the parent entity linking to its water surface child.
#[derive(Component)]
pub struct PoolWaterLink(pub Entity);

/// Marker on the child water surface entity linking back to its pool parent.
#[derive(Component)]
pub struct PoolWaterSurface(pub Entity);

/// Interactive pool water.
/// Attach to any mesh entity (e.g. a cube) to turn it into a pool.
/// A water surface child entity is spawned automatically inside it.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[serde(crate = "renzora::serde")]
#[reflect(Component, Default)]
pub struct PoolWater {
    /// How far below the top face the water sits (0 = flush with top, 0.1 = slightly below)
    pub water_level: f32,
    /// Index of refraction (1.333 = water)
    pub ior: f32,
    /// Minimum Fresnel reflectance (0–1)
    pub fresnel_min: f32,
    /// Caustic brightness multiplier
    pub caustic_intensity: f32,
    /// Deep water absorption color
    pub deep_color: [f32; 3],
    /// Shallow water tint
    pub shallow_color: [f32; 3],
    /// Foam color
    pub foam_color: [f32; 3],
    /// Simulation damping (0.99–0.999, higher = longer ripples)
    pub damping: f32,
    /// Wave propagation speed
    pub wave_speed: f32,
    /// Height scale (maps sim values to world units)
    pub height_scale: f32,
    /// Simulation resolution (width = height)
    pub sim_resolution: u32,
    /// Mesh subdivisions
    pub mesh_subdivisions: u32,
    /// Sun specular power
    pub specular_power: f32,
    /// Refraction UV distortion strength
    pub refraction_strength: f32,
    /// Maximum depth for absorption (world units)
    pub max_depth: f32,
    /// Absorption coefficients (R, G, B) — higher = absorbed faster
    pub absorption_r: f32,
    pub absorption_g: f32,
    pub absorption_b: f32,
    /// Shoreline foam depth threshold
    pub foam_depth: f32,
}

impl Default for PoolWater {
    fn default() -> Self {
        Self {
            water_level: 0.05,
            ior: 1.333,
            fresnel_min: 0.02,
            caustic_intensity: 0.25,
            deep_color: [0.005, 0.02, 0.08],
            shallow_color: [0.04, 0.22, 0.28],
            foam_color: [0.9, 0.92, 0.95],
            damping: 0.995,
            wave_speed: 2.0,
            height_scale: 0.3,
            sim_resolution: 256,
            mesh_subdivisions: 200,
            specular_power: 5000.0,
            refraction_strength: 0.03,
            max_depth: 5.0,
            absorption_r: 3.0,
            absorption_g: 1.0,
            absorption_b: 0.4,
            foam_depth: 1.0,
        }
    }
}

// ── Mesh generation ───────────────────────────────────────────────────────────

fn generate_pool_water_mesh(half_x: f32, half_z: f32, subdivisions: u32) -> Mesh {
    let verts_per_edge = subdivisions + 1;
    let total_verts = (verts_per_edge * verts_per_edge) as usize;
    let total_indices = (subdivisions * subdivisions * 6) as usize;

    let mut positions = Vec::with_capacity(total_verts);
    let mut normals = Vec::with_capacity(total_verts);
    let mut uvs = Vec::with_capacity(total_verts);
    let mut indices = Vec::with_capacity(total_indices);

    for z in 0..verts_per_edge {
        for x in 0..verts_per_edge {
            let fx = x as f32 / subdivisions as f32;
            let fz = z as f32 / subdivisions as f32;
            positions.push([
                -half_x + fx * half_x * 2.0,
                0.0,
                -half_z + fz * half_z * 2.0,
            ]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([fx, fz]);
        }
    }

    for z in 0..subdivisions {
        for x in 0..subdivisions {
            let tl = z * verts_per_edge + x;
            let tr = tl + 1;
            let bl = tl + verts_per_edge;
            let br = bl + 1;
            indices.push(tl);
            indices.push(bl);
            indices.push(tr);
            indices.push(tr);
            indices.push(bl);
            indices.push(br);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

// ── Systems ───────────────────────────────────────────────────────────────────

/// Auto-setup: spawn a water surface child entity inside the pool container.
fn setup_pool_water(
    mut commands: Commands,
    query: Query<(Entity, &PoolWater), Without<PoolWaterLink>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<PoolWaterMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    for (entity, pool) in query.iter() {
        // Unit-sized plane (0.5 half-extents) — inherits parent's XZ scale
        let mesh = meshes.add(generate_pool_water_mesh(0.5, 0.5, pool.mesh_subdivisions));

        let sim = WaterSim::new(
            pool.sim_resolution as usize,
            pool.sim_resolution as usize,
            pool.damping,
            pool.wave_speed,
            &mut images,
        );

        let mat = materials.add(PoolWaterMaterial {
            uniforms: PoolWaterUniforms::default(),
            heightfield: Some(sim.texture_handle.clone()),
        });

        // Spawn water surface as a child, positioned at the top face of the pool.
        // Uses a unit-sized plane (1x1) so it inherits the parent's XZ scale.
        // Positioned at y=0.5 (top of a unit cube) with a tiny offset to avoid z-fight.
        let surface_id = commands
            .spawn((
                Name::new("Water Surface"),
                Mesh3d(mesh),
                MeshMaterial3d(mat),
                Transform::from_translation(Vec3::new(0.0, 0.5 - pool.water_level, 0.0)),
                sim,
                PoolWaterSurface(entity),
            ))
            .set_parent_in_place(entity)
            .id();

        commands.entity(entity).insert(PoolWaterLink(surface_id));
    }
}

/// Clean up water surface when PoolWater is removed.
fn cleanup_pool_water(
    mut commands: Commands,
    mut removed: RemovedComponents<PoolWater>,
    links: Query<&PoolWaterLink>,
) {
    for entity in removed.read() {
        if let Ok(link) = links.get(entity) {
            if let Ok(mut ec) = commands.get_entity(link.0) {
                ec.despawn();
            }
        }
        if let Ok(mut ec) = commands.get_entity(entity) {
            ec.remove::<PoolWaterLink>();
        }
    }
}

/// Ensure cameras have DepthPrepass for depth-based water effects.
fn ensure_depth_prepass(
    mut commands: Commands,
    cameras: Query<Entity, (With<Camera3d>, Without<DepthPrepass>)>,
    pool_exists: Query<(), With<PoolWater>>,
) {
    if pool_exists.is_empty() {
        return;
    }
    for entity in cameras.iter() {
        commands.entity(entity).try_insert(DepthPrepass);
    }
}

/// Step simulation, upload texture, sync uniforms every frame.
fn update_pool_water(
    time: Res<Time>,
    pool_query: Query<(&PoolWater, &PoolWaterLink)>,
    mut surface_query: Query<(
        &mut WaterSim,
        &MeshMaterial3d<PoolWaterMaterial>,
        &PoolWaterSurface,
    )>,
    mut materials: ResMut<Assets<PoolWaterMaterial>>,
    mut images: ResMut<Assets<Image>>,
    sun_query: Query<&GlobalTransform, With<DirectionalLight>>,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();

    let sun_dir = sun_query
        .iter()
        .next()
        .map(|tr| tr.forward().as_vec3())
        .unwrap_or(Vec3::new(-0.3, -0.7, -0.4).normalize());

    for (mut sim, mat_handle, surface) in surface_query.iter_mut() {
        let Ok((pool, _)) = pool_query.get(surface.0) else {
            continue;
        };

        // Step simulation
        sim.step();
        sim.upload(&mut images);

        // Add ambient ripples (very subtle)
        if (t * 2.0).fract() < dt * 2.0 {
            let rx = hash_f32(t * 13.7) * 0.8 + 0.1;
            let ry = hash_f32(t * 17.3) * 0.8 + 0.1;
            sim.add_drop(rx, ry, 0.03, 0.01);
        }

        // Sync material uniforms
        if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
            mat.uniforms.light_direction = Vec4::new(sun_dir.x, sun_dir.y, sun_dir.z, 0.0);
            mat.uniforms.ior = pool.ior;
            mat.uniforms.fresnel_min = pool.fresnel_min;
            mat.uniforms.caustic_intensity = pool.caustic_intensity;
            mat.uniforms.time = t;
            mat.uniforms.height_scale = pool.height_scale;
            mat.uniforms.specular_power = pool.specular_power;
            mat.uniforms.refraction_strength = pool.refraction_strength;
            mat.uniforms.max_depth = pool.max_depth;

            let dc = pool.deep_color;
            mat.uniforms.deep_color = Vec4::new(dc[0], dc[1], dc[2], 1.0);
            let sc = pool.shallow_color;
            mat.uniforms.shallow_color = Vec4::new(sc[0], sc[1], sc[2], 1.0);
            let fc = pool.foam_color;
            mat.uniforms.foam_color = Vec4::new(fc[0], fc[1], fc[2], 1.0);
            mat.uniforms.absorption = Vec4::new(
                pool.absorption_r,
                pool.absorption_g,
                pool.absorption_b,
                pool.foam_depth,
            );
        }
    }
}

/// Public API: add a drop to a specific pool water entity.
pub fn add_drop_to_pool(
    entity: Entity,
    sim_query: &mut Query<&mut WaterSim>,
    uv_x: f32,
    uv_y: f32,
    radius: f32,
    strength: f32,
) {
    if let Ok(mut sim) = sim_query.get_mut(entity) {
        sim.add_drop(uv_x, uv_y, radius, strength);
    }
}

fn hash_f32(x: f32) -> f32 {
    let s = (x * 127.1 + 311.7).sin() * 43_758.547;
    s.fract()
}

// ── Inspector ─────────────────────────────────────────────────────────────────


// ── Plugin ────────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct PoolWaterPlugin;

impl Plugin for PoolWaterPlugin {
    fn build(&self, app: &mut App) {
        info!("[pool_water] native plugin");

        embedded_asset!(app, "pool_water.wgsl");

        app.add_plugins(MaterialPlugin::<PoolWaterMaterial>::default())
            .register_type::<PoolWater>()
            .add_systems(
                Update,
                (
                    ensure_depth_prepass,
                    setup_pool_water,
                    update_pool_water,
                    cleanup_pool_water,
                ),
            );

        inspector::register(app);
    }
}

// `Runtime`, explicitly. `plugin!` defaults to `Editor` where `add!` defaulted
// to `Runtime`, so omitting this would quietly stop shipping the water to games.
renzora::plugin!(PoolWaterPlugin, Runtime);
