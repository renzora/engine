//! Main-world half of the water system: mesh/material setup, driving the GPU
//! simulation's clock, and keeping the CPU height field in step with it.

use bevy::camera::primitives::Aabb;
use bevy::math::Vec3A;
use bevy::prelude::*;

use crate::buoyancy::Buoyant;
use crate::component::{WaterMeshMode, WaterSurface, WaveCascade, MAX_CASCADES};
use crate::heightfield::{spectrum_seed, WaterHeightField};
use crate::material::{sync_uniforms, WaterMaterial};
use crate::mesh::{generate_clipmap_mesh, generate_water_mesh};
use crate::sim::{create_cascade_textures, CascadeGpu, WaterSimParams, WaterSimTextures};

/// How often the CPU height field is rebuilt. Buoyancy integrates over many
/// frames anyway, so it does not need the simulation's full rate.
const HEIGHTFIELD_HZ: f32 = 20.0;

/// Vertical padding on the water's culling bounds, in metres. Comfortably above
/// the displacement any sane sea state produces.
const WAVE_HEIGHT_MARGIN: f32 = 32.0;

/// The parameters that, when changed, mean the time-independent spectrum has to
/// be rebuilt.
///
/// Deliberately *not* the whole `WaveCascade`. `time_scale` only rescales the
/// propagation clock, and `displacement_scale` / `normal_scale` are material
/// multipliers applied after the transform; rebuilding for those would restart
/// the sea (and re-run the CPU mirror) on every frame of a slider drag.
/// `whitecap` and the foam rates are re-uploaded to the unpack kernel each step
/// anyway, so they need no rebuild either.
#[derive(Clone, PartialEq)]
struct CascadeSignature {
    tile_length: Vec2,
    wind_speed: f32,
    wind_direction: f32,
    fetch_length: f32,
    swell: f32,
    spread: f32,
    detail: f32,
}

impl CascadeSignature {
    fn of(cascade: &WaveCascade) -> Self {
        Self {
            tile_length: cascade.tile_length,
            wind_speed: cascade.wind_speed,
            wind_direction: cascade.wind_direction,
            fetch_length: cascade.fetch_length,
            swell: cascade.swell,
            spread: cascade.spread,
            detail: cascade.detail,
        }
    }
}

#[derive(Clone, PartialEq)]
struct SpectrumSignature {
    cascades: Vec<CascadeSignature>,
    sea_depth: f32,
    seed: u32,
    map_size: u32,
}

impl SpectrumSignature {
    fn of(surface: &WaterSurface) -> Self {
        Self {
            cascades: surface
                .active_cascades()
                .iter()
                .map(CascadeSignature::of)
                .collect(),
            sea_depth: surface.sea_depth,
            seed: surface.seed,
            map_size: surface.clamped_map_size(),
        }
    }
}

/// Everything the simulation needs to remember between frames.
#[derive(Resource, Default)]
pub struct WaterSimState {
    /// Per-cascade simulation clock. Cascades start at staggered times so their
    /// wave fields never line up into a visible beat.
    pub cascade_times: Vec<f32>,
    /// Wall clock for the update throttle.
    time: f32,
    next_update_time: f32,
    /// Frames left to keep asking for a spectrum rebuild. Held for a few frames
    /// because the render pass silently skips work until its pipelines finish
    /// compiling, and a one-frame flag can land in that window and be lost.
    dirty_frames: u8,
    signature: Option<SpectrumSignature>,
    heightfield_timer: f32,
}

/// The mesh a water entity was last built with, so it is only rebuilt when one
/// of these actually changes.
#[derive(Component, Clone, PartialEq)]
pub struct WaterMeshSpec {
    mode: WaterMeshMode,
    size: f32,
    subdivisions: u32,
    rings: u32,
    resolution: u32,
    quad_size: f32,
}

impl WaterMeshSpec {
    fn of(surface: &WaterSurface) -> Self {
        // Resolved through `clipmap_params`, so switching the quality preset
        // changes the spec and rebuilds the mesh — reading the raw fields here
        // would leave a preset change invisible until something else moved.
        let (rings, resolution, quad_size) = surface.clipmap_params();
        Self {
            mode: surface.mesh_mode,
            size: surface.mesh_size,
            subdivisions: surface.subdivisions,
            rings,
            resolution,
            quad_size,
        }
    }

    fn build(&self) -> Mesh {
        match self.mode {
            WaterMeshMode::Grid => generate_water_mesh(self.size, self.subdivisions),
            WaterMeshMode::Clipmap => {
                generate_clipmap_mesh(self.rings, self.resolution, self.quad_size)
            }
        }
    }

    /// Bounds for culling. The mesh is flat, but the vertex shader lifts it by
    /// metres of wave, so the automatic (zero-height) AABB would let a wave
    /// crest be culled while it is still on screen — most visible with the
    /// camera down near the water, which is exactly where waves matter.
    fn bounds(&self) -> Aabb {
        let half = match self.mode {
            WaterMeshMode::Grid => self.size * 0.5,
            WaterMeshMode::Clipmap => {
                self.resolution as f32 * self.quad_size * 0.5 * (1u32 << self.rings.min(16)) as f32
            }
        };
        Aabb {
            center: Vec3A::ZERO,
            half_extents: Vec3A::new(half, WAVE_HEIGHT_MARGIN, half),
        }
    }
}

/// Create (and re-create) the cascade maps. Runs before the entity setup so a
/// freshly added water surface picks up its textures on the same frame.
pub fn ensure_cascade_textures(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    existing: Option<Res<WaterSimTextures>>,
    water: Query<&WaterSurface>,
) {
    let Some(surface) = water.iter().next() else {
        return;
    };
    let map_size = surface.clamped_map_size();
    let num_cascades = surface.active_cascades().len().max(1) as u32;

    if let Some(existing) = existing.as_deref() {
        if existing.map_size == map_size && existing.num_cascades == num_cascades {
            return;
        }
    }

    let (displacement, normal) = create_cascade_textures(&mut images, map_size, num_cascades);
    commands.insert_resource(WaterSimTextures {
        displacement,
        normal,
        map_size,
        num_cascades,
    });
}

/// Water entities whose configuration changed this frame, with whatever mesh
/// and mesh-spec they already carry. Aliased to keep clippy's
/// `type_complexity` lint (denied in CI) off the system signature.
type ChangedWaterSurfaces<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static WaterSurface,
        Option<&'static WaterMeshSpec>,
        Option<&'static Mesh3d>,
    ),
    Changed<WaterSurface>,
>;

/// Auto-setup: when a `WaterSurface` is added without a mesh, generate the mesh
/// + material. Also rebuilds the mesh when its parameters change.
pub fn setup_water_entities(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
    textures: Option<Res<WaterSimTextures>>,
    query: ChangedWaterSurfaces,
) {
    for (entity, surface, spec, mesh) in query.iter() {
        let wanted = WaterMeshSpec::of(surface);
        if mesh.is_some() && spec == Some(&wanted) {
            continue;
        }

        let mesh_handle = meshes.add(wanted.build());
        let material = materials.add(WaterMaterial {
            displacements: textures.as_ref().map(|t| t.displacement.clone()),
            normals: textures.as_ref().map(|t| t.normal.clone()),
            ..default()
        });
        commands
            .entity(entity)
            .try_insert((
                Mesh3d(mesh_handle),
                MeshMaterial3d(material),
                wanted.bounds(),
                wanted,
            ));
    }
}

/// Advance the simulation clock and publish this frame's GPU parameters.
pub fn drive_water_simulation(
    time: Res<Time>,
    mut state: ResMut<WaterSimState>,
    mut params: ResMut<WaterSimParams>,
    mut heightfield: ResMut<WaterHeightField>,
    water: Query<&WaterSurface>,
) {
    let Some(surface) = water.iter().next() else {
        params.uniform.num_cascades = 0;
        params.step = false;
        return;
    };
    let cascades = surface.active_cascades();
    if cascades.is_empty() {
        params.uniform.num_cascades = 0;
        params.step = false;
        return;
    }

    // Re-seed the per-cascade clocks whenever the cascade count changes. The
    // 120 s head start keeps the sea from starting perfectly flat, and the
    // PI * i stagger stops cascades interfering.
    if state.cascade_times.len() != cascades.len() {
        state.cascade_times = (0..cascades.len())
            .map(|i| 120.0 + std::f32::consts::PI * i as f32)
            .collect();
    }

    let signature = SpectrumSignature::of(surface);
    if state.signature.as_ref() != Some(&signature) {
        state.signature = Some(signature);
        state.dirty_frames = 3;
        heightfield.invalidate();
    }

    // Update throttle, mirroring the reference: catch up on the drift so the
    // simulation advances by real time even at a low update rate.
    let delta = time.delta_secs();
    let ups = surface.updates_per_second.max(0.0);
    let step = ups == 0.0 || state.time >= state.next_update_time;
    let update_delta = if step {
        let target = 1.0 / (ups + 1e-10);
        let advance = if ups == 0.0 {
            delta
        } else {
            (target + (state.time - state.next_update_time)).clamp(0.0, 1.0)
        };
        state.next_update_time = state.time + target;
        advance
    } else {
        0.0
    };
    state.time += delta;

    if step {
        // Each cascade runs its own clock at its own rate, so a long swell can
        // roll slowly under a fast chop.
        for (slot, cascade) in state.cascade_times.iter_mut().zip(cascades) {
            *slot += update_delta * cascade.time_scale;
        }
    }

    params.uniform.map_size = surface.clamped_map_size();
    params.uniform.num_cascades = cascades.len() as u32;
    params.uniform.depth = surface.sea_depth.max(0.1);
    params.uniform.cascades = [CascadeGpu::default(); MAX_CASCADES];
    for (i, cascade) in cascades.iter().enumerate() {
        params.uniform.cascades[i] = CascadeGpu {
            tile_length: cascade.tile_length.max(Vec2::splat(1e-3)),
            alpha: cascade.jonswap_alpha(),
            peak_frequency: cascade.jonswap_peak_frequency(),
            wind_speed: cascade.wind_speed.max(1e-4),
            angle: cascade.wind_direction,
            swell: cascade.swell,
            detail: cascade.detail,
            spread: cascade.spread,
            time: state.cascade_times[i],
            whitecap: cascade.whitecap,
            foam_grow_rate: cascade.foam_grow_rate(update_delta),
            foam_decay_rate: cascade.foam_decay_rate(update_delta),
            pad: 0.0,
            // Per cascade, not per surface — see `spectrum_seed`.
            seed: spectrum_seed(surface.seed, i),
        };
    }

    params.step = step;
    params.regenerate_spectrum = state.dirty_frames > 0;
    state.dirty_frames = state.dirty_frames.saturating_sub(1);
}

/// Keep the clipmap centred on the camera, snapped to its finest quad so the
/// world-space wave UVs don't crawl across the surface as it moves.
///
/// Which camera counts, in order: the play-mode game camera, then the focused
/// editor viewport camera, then any active 3D camera. Without that order a
/// scene with several render targets — a material preview sphere, say — could
/// drag the whole ocean off to wherever that preview camera happens to sit.
pub fn follow_camera_with_clipmap(
    mut water: Query<(&WaterSurface, &mut Transform)>,
    play_camera: Query<&GlobalTransform, With<renzora::PlayModeCamera>>,
    editor_camera: Query<&GlobalTransform, With<renzora::EditorCamera>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
) {
    let fallback = || {
        cameras
            .iter()
            .find(|(camera, _)| camera.is_active)
            .map(|(_, transform)| transform.translation())
    };
    let Some(camera_pos) = play_camera
        .iter()
        .next()
        .or_else(|| editor_camera.iter().next())
        .map(|transform| transform.translation())
        .or_else(fallback)
    else {
        return;
    };

    for (surface, mut transform) in water.iter_mut() {
        if surface.mesh_mode != WaterMeshMode::Clipmap {
            continue;
        }
        // The *resolved* quad size: snapping to the raw field while the mesh
        // was built from a quality preset would snap to a grid the vertices
        // don't sit on, and the wave UVs would crawl by the difference.
        let snap = surface.clipmap_params().2.max(0.01);
        let snapped_x = (camera_pos.x / snap).floor() * snap;
        let snapped_z = (camera_pos.z / snap).floor() * snap;
        if transform.translation.x != snapped_x || transform.translation.z != snapped_z {
            transform.translation.x = snapped_x;
            transform.translation.z = snapped_z;
        }
    }
}

/// Push shading parameters, the sun, and the current cascade maps into each
/// water material.
pub fn update_water_uniforms(
    mut materials: ResMut<Assets<WaterMaterial>>,
    textures: Option<Res<WaterSimTextures>>,
    water_query: Query<(&WaterSurface, &MeshMaterial3d<WaterMaterial>)>,
    sun_query: Query<(&GlobalTransform, &DirectionalLight)>,
) {
    let (sun_dir, sun_intensity) = sun_query
        .iter()
        .next()
        .map(|(transform, light)| {
            let illuminance = light.illuminance / 10000.0;
            (transform.forward().as_vec3(), illuminance.clamp(0.0, 1.0))
        })
        .unwrap_or((Vec3::new(-0.3, -0.7, -0.4).normalize(), 1.0));

    for (surface, mat_handle) in water_query.iter() {
        let Some(mut material) = materials.get_mut(&mat_handle.0) else {
            continue;
        };
        sync_uniforms(surface, &mut material.uniforms);
        material.uniforms.sun_direction =
            Vec4::new(sun_dir.x, sun_dir.y, sun_dir.z, sun_intensity);

        // Re-point at the cascade maps if they were re-created (resolution or
        // cascade-count change). Comparing first keeps this from marking the
        // material changed on every frame.
        if let Some(textures) = textures.as_ref() {
            if material.displacements.as_ref() != Some(&textures.displacement) {
                material.displacements = Some(textures.displacement.clone());
            }
            if material.normals.as_ref() != Some(&textures.normal) {
                material.normals = Some(textures.normal.clone());
            }
        }
    }
}

/// Rebuild the CPU height field on a throttle — only while something is
/// actually floating, since nothing else reads it yet.
pub fn update_water_heightfield(
    time: Res<Time>,
    mut state: ResMut<WaterSimState>,
    mut field: ResMut<WaterHeightField>,
    water: Query<&WaterSurface>,
    buoyant: Query<(), With<Buoyant>>,
) {
    if buoyant.is_empty() {
        return;
    }
    let Some(surface) = water.iter().next() else {
        return;
    };

    state.heightfield_timer -= time.delta_secs();
    if state.heightfield_timer > 0.0 && field.ready {
        return;
    }
    state.heightfield_timer = 1.0 / HEIGHTFIELD_HZ;

    let times = state.cascade_times.clone();
    field.update(surface, &times);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::WaterSimParams;
    use std::time::Duration;

    /// A minimal app with just the clock-driving system — no render world, so
    /// this runs headless in CI.
    fn app_with(surface: WaterSurface) -> App {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<WaterSimState>()
            .init_resource::<WaterSimParams>()
            .init_resource::<WaterHeightField>()
            .add_systems(Update, drive_water_simulation);
        app.world_mut().spawn(surface);
        app
    }

    fn cascade_times(app: &App) -> Vec<f32> {
        app.world().resource::<WaterSimState>().cascade_times.clone()
    }

    #[test]
    fn time_scale_scales_each_cascades_clock() {
        let mut surface = WaterSurface::default();
        // 0 updates/second means "every frame", so update_delta is the frame
        // delta and the arithmetic below is exact.
        surface.updates_per_second = 0.0;
        surface.cascades[0].time_scale = 0.0;
        surface.cascades[1].time_scale = 1.0;
        surface.cascades[2].time_scale = 2.0;

        let mut app = app_with(surface);
        app.update();
        let start = cascade_times(&app);

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(100));
        app.update();
        let now = cascade_times(&app);

        let advanced: Vec<f32> = now.iter().zip(&start).map(|(a, b)| a - b).collect();
        assert!(
            advanced[0].abs() < 1e-6,
            "time_scale 0 should freeze the cascade, advanced by {}",
            advanced[0]
        );
        assert!(
            (advanced[1] - 0.1).abs() < 1e-4,
            "time_scale 1 should track real time, advanced by {}",
            advanced[1]
        );
        assert!(
            (advanced[2] - 0.2).abs() < 1e-4,
            "time_scale 2 should double, advanced by {}",
            advanced[2]
        );
    }

    #[test]
    fn each_cascade_gets_its_own_seed() {
        // The uniform the GPU reads must carry a distinct seed per cascade —
        // the CPU mirror is covered separately in `heightfield`, but this is
        // the path the *rendered* ocean actually takes.
        let mut app = app_with(WaterSurface::default());
        app.update();

        let params = app.world().resource::<WaterSimParams>();
        let seeds: Vec<IVec2> = (0..params.uniform.num_cascades as usize)
            .map(|i| params.uniform.cascades[i].seed)
            .collect();
        assert!(seeds.len() >= 2);
        for (i, seed) in seeds.iter().enumerate() {
            for (j, other) in seeds.iter().enumerate().skip(i + 1) {
                assert_ne!(seed, other, "cascades {i} and {j} share a spectrum seed");
            }
        }
    }

    #[test]
    fn time_scale_alone_does_not_rebuild_the_spectrum() {
        // Dragging the Time Scale slider must not re-roll the sea. It only
        // rescales the propagation clock, so it is deliberately absent from
        // `CascadeSignature` — if it creeps back in, the ocean restarts (and
        // the CPU mirror re-runs) on every frame of the drag.
        let mut app = app_with(WaterSurface::default());
        app.update();
        // `dirty_frames` holds the initial rebuild for a few frames; drain it.
        for _ in 0..4 {
            app.update();
        }
        assert!(!app.world().resource::<WaterSimParams>().regenerate_spectrum);

        let entity = app
            .world_mut()
            .query_filtered::<Entity, With<WaterSurface>>()
            .single(app.world())
            .unwrap();
        app.world_mut()
            .get_mut::<WaterSurface>(entity)
            .unwrap()
            .cascades[0]
            .time_scale = 0.25;
        app.update();

        assert!(
            !app.world().resource::<WaterSimParams>().regenerate_spectrum,
            "changing time_scale rebuilt the spectrum"
        );
    }

    #[test]
    fn wind_speed_does_rebuild_the_spectrum() {
        // The counterpart: a genuine sea-state change must still invalidate.
        let mut app = app_with(WaterSurface::default());
        for _ in 0..5 {
            app.update();
        }
        assert!(!app.world().resource::<WaterSimParams>().regenerate_spectrum);

        let entity = app
            .world_mut()
            .query_filtered::<Entity, With<WaterSurface>>()
            .single(app.world())
            .unwrap();
        app.world_mut()
            .get_mut::<WaterSurface>(entity)
            .unwrap()
            .cascades[0]
            .wind_speed = 18.0;
        app.update();

        assert!(app.world().resource::<WaterSimParams>().regenerate_spectrum);
    }
}

/// Spawn a water surface entity with the given configuration.
pub fn spawn_water(
    commands: &mut Commands,
    config: WaterSurface,
    transform: Transform,
) -> Entity {
    // The mesh and material are attached by `setup_water_entities` on the next
    // run, once the cascade maps exist.
    commands
        .spawn((Name::new("Water Surface"), transform, config))
        .id()
}
