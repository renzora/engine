//! The GPU backend's main-world half: render children, buffers, and the
//! per-frame hand-off of planned steps to the render world.
//!
//! A GPU emitter's render child is spawned like a CPU one ([`crate::render`]):
//! a hidden child of the effect, on the effect's render layers, outside
//! selection bounds, with its own `Aabb`. What it draws is a mesh of one quad
//! per pool slot (shared between every emitter of that capacity) with a
//! [`GpuParticleMaterial3d`] or [`GpuParticleMaterial2d`] binding the pool.
//! It carries a [`GpuEmitterLink`], which is everything the render world
//! needs: the buffers, the shader, the data table and this frame's batch of
//! steps.
//!
//! ## Frames and batches
//!
//! Every frame [`sync_gpu_emitters`] moves each GPU emitter's planned steps
//! into its link as a new batch, numbered by a frame counter, and the render
//! world appends a batch it has not seen to its own queue. A batch is never
//! dropped when the render world cannot run it yet (its pipeline is still
//! compiling, its buffers not yet uploaded): the queue holds it until it can,
//! so a GPU emitter that starts late starts from the right state.
//!
//! ## Readback
//!
//! The first words of each state buffer (live count, drops, the reset
//! serial, light sums, bounds) are read back with Bevy's `Readback` every
//! frame. They arrive a frame late, which is fine for a stats chip, a light
//! that is smoothed over a tenth of a second anyway, and culling bounds that
//! are padded by how far the fastest particle moves meanwhile.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::ecs::system::SystemParam;
use bevy::math::Affine3A;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::storage::ShaderBuffer;

use super::codegen::{generate_with, GpuProgram};
use super::layout::{self, GpuReadout};
#[cfg(feature = "render_2d")]
use super::material::GpuParticleMaterial2d;
#[cfg(feature = "render_3d")]
use super::material::GpuParticleMaterial3d;
use crate::asset::{Dimension, DrawOrder, RenderSettings, RibbonKind, SimulationSpace};
use crate::render::{material::ParticleMaterialParams, sim_to_mesh, sort_eye, spawn_renderer, RendererAssets};
use crate::sim::{FrameInput, GpuStepPlan};
use crate::systems::ParticleRuntime;

/// On a GPU emitter's render child: what the render world simulates.
#[derive(Component, Clone)]
pub struct GpuEmitterLink {
    pub particles: Handle<ShaderBuffer>,
    pub state: Handle<ShaderBuffer>,
    /// The emitter's generated kernels, shared by every emitter of the same
    /// graph structure.
    pub shader: Handle<Shader>,
    pub capacity: u32,
    pub seed: u64,
    /// What the frame's sort orders the particles by (`layout::SORT_*`).
    pub sort_mode: u32,
    /// A `Connect` ribbon's strips.
    pub ribbon_groups: u32,
    /// The effect's sub-emitter event buffer, shared by every GPU emitter of
    /// a group; a small one of its own for an emitter in none.
    pub events: Handle<ShaderBuffer>,
    /// The effect entity, for an emitter that shares events with others:
    /// the render world steps such emitters together. `None` alone.
    pub group: Option<Entity>,
    /// Place in the group's topological order, parents first.
    pub order: u32,
    /// This emitter's inbox in `events`, when other GPU emitters spawn it.
    pub inbox: Option<(u32, u32)>,
    /// Constants, curve keys and gradient stops ([`layout::pack_data`]).
    pub data: Arc<Vec<[f32; 4]>>,
    /// The frame this batch was made in; the render world takes each once.
    pub frame: u64,
    /// Empty the pool before this batch's steps.
    pub reset: bool,
    pub serial: u32,
    pub steps: Vec<GpuStepPlan>,
    /// This frame's transform and wind, for the frame record.
    pub frame_input: FrameInput,
    /// The sorting camera in simulation space.
    pub camera: Vec3,
    /// Simulation space to the render child's mesh space.
    pub to_mesh: Affine3A,
    /// How much further than a quad a particle reaches, for the culling
    /// bounds: 1 for quads, more for mesh copies.
    pub bounds_pad: f32,
}

/// The newest readback of a GPU emitter's counters.
#[derive(Component, Default)]
pub struct GpuReadoutCell(pub Option<GpuReadout>);

/// Generated shaders by structure hash, so every rebuild of an edited effect
/// whose structure did not change reuses the compiled pipeline.
#[derive(Resource, Default)]
pub struct GpuShaderCache(pub HashMap<u64, Handle<Shader>>);

/// One quad per slot, per capacity: every emitter of a capacity draws the
/// same mesh, since the slot is all it says.
#[derive(Resource, Default)]
pub struct GpuQuadMeshes(pub HashMap<u32, Handle<Mesh>>);

/// Ribbon pool meshes by shape ([`RibbonMesh`]): like the quads, the mesh
/// says only which vertex of which strip each vertex is, so every emitter of
/// one shape shares it.
#[derive(Resource, Default)]
pub struct GpuRibbonMeshes(pub HashMap<RibbonMesh, Handle<Mesh>>);

/// The most vertices a ribbon pool mesh holds. A trail of 16 points on a
/// pool of 200,000 is 6.8 M; past this the pool draws only as many strips as
/// fit, with a Console line saying so, as the mesh-particle pool does.
pub const MAX_GPU_RIBBON_VERTICES: usize = 8 << 20;

/// The shape of a GPU ribbon's pool mesh. Each vertex's position is
/// `(across, row, strip)`, which the particle shader's ribbon fetch turns
/// into the point it stands for: `across` the side of a flat strip (-0.5 or
/// 0.5) or the way round a tube (0 to 1, less a half); for a `Trail`, `row`
/// the point along the strip (0 the particle, then its history newest
/// first) and `strip` the pool slot; for a `Connect` ribbon, `row` 0 or 1,
/// the two ends of segment `strip`, which joins sorted ranks `strip` and
/// `strip + 1`. A Connect segment is its own four (or ring of) vertices,
/// not a shared row, so the shader can drop the segment between one strip's
/// last particle and the next strip's first without the mesh knowing where
/// strips end.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RibbonMesh {
    pub connect: bool,
    /// Points along a strip: a trail's points plus the particle, or 2 for a
    /// Connect segment.
    pub rows: u32,
    /// Vertices across: 2 for a flat strip, `segments + 1` round a tube.
    pub cols: u32,
    /// Strips: pool slots, or Connect segments.
    pub strips: u32,
}

impl RibbonMesh {
    /// The pool mesh for `render`'s ribbon on a pool of `capacity`.
    pub fn for_render(render: &RenderSettings, dimension: Dimension, capacity: u32) -> Self {
        let r = &render.ribbon;
        let cols = if render.draws_tube(dimension) { r.tube_segments() + 1 } else { 2 };
        match r.kind {
            RibbonKind::Trail => Self { connect: false, rows: r.trail_points() as u32 + 1, cols, strips: capacity.max(1) },
            RibbonKind::Connect => Self { connect: true, rows: 2, cols, strips: capacity.saturating_sub(1).max(1) },
        }
    }

    pub fn vertices_per_strip(&self) -> usize {
        (self.rows * self.cols) as usize
    }

    /// The strips that fit under [`MAX_GPU_RIBBON_VERTICES`].
    pub fn capped(mut self) -> Self {
        let fit = (MAX_GPU_RIBBON_VERTICES / self.vertices_per_strip().max(1)).max(1) as u32;
        self.strips = self.strips.min(fit);
        self
    }

    pub fn build(&self) -> Mesh {
        let per = self.vertices_per_strip();
        let mut positions = Vec::with_capacity(per * self.strips as usize);
        let quads = ((self.rows - 1) * (self.cols - 1)) as usize;
        let mut indices = Vec::with_capacity(quads * 6 * self.strips as usize);
        let across = |c: u32| -> f32 {
            if self.cols == 2 {
                if c == 0 {
                    -0.5
                } else {
                    0.5
                }
            } else {
                c as f32 / (self.cols - 1) as f32 - 0.5
            }
        };
        for strip in 0..self.strips {
            let base = strip * self.rows * self.cols;
            for row in 0..self.rows {
                for c in 0..self.cols {
                    positions.push([across(c), row as f32, strip as f32]);
                }
            }
            for row in 0..self.rows - 1 {
                for c in 0..self.cols - 1 {
                    let a = base + row * self.cols + c;
                    let b = a + 1;
                    let d = a + self.cols;
                    let e = d + 1;
                    indices.extend_from_slice(&[a, b, e, a, e, d]);
                }
            }
        }
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_indices(Indices::U32(indices));
        mesh
    }
}

/// Counts main-world frames, numbering batches.
#[derive(Resource, Default)]
pub struct GpuFrameCounter(pub u64);

/// Register the GPU backend: its materials, its systems around the CPU
/// runtime's, the readback observer and the render world's half. Called by
/// `ParticlesPlugin` on every build that compiles the backend in.
pub fn plugin(app: &mut App) {
    use crate::systems::{step_effects, sync_instances, write_meshes};
    app.init_resource::<GpuShaderCache>()
        .init_resource::<GpuQuadMeshes>()
        .init_resource::<GpuRibbonMeshes>()
        .init_resource::<GpuFrameCounter>();
    #[cfg(feature = "render_3d")]
    app.add_plugins(bevy::pbr::MaterialPlugin::<GpuParticleMaterial3d>::default()).add_systems(
        PostUpdate,
        (
            crate::render::sync_motion_rate::<GpuParticleMaterial3d>.after(step_effects).in_set(crate::ParticleSystems),
            crate::render::material::resolve_render_routes::<GpuParticleMaterial3d>.after(crate::ParticleSystems),
        ),
    );
    #[cfg(feature = "render_2d")]
    app.add_plugins(bevy::sprite_render::Material2dPlugin::<GpuParticleMaterial2d>::default());
    app.add_systems(
        PostUpdate,
        (
            (attach_gpu_renderers, apply_gpu_readouts).chain().after(sync_instances).before(step_effects),
            sync_gpu_emitters.after(step_effects).before(write_meshes),
        )
            .in_set(crate::ParticleSystems),
    );
    app.add_observer(on_readback);
    super::render::build(app);
}

/// Everything spawning a GPU render child takes, beside the mesh assets it
/// shares with the CPU renderer's [`RendererAssets`].
#[derive(SystemParam)]
pub struct GpuRendererAssets<'w> {
    pub buffers: ResMut<'w, Assets<ShaderBuffer>>,
    pub shaders: ResMut<'w, Assets<Shader>>,
    pub asset_server: Res<'w, AssetServer>,
    pub shader_cache: ResMut<'w, GpuShaderCache>,
    pub quads: ResMut<'w, GpuQuadMeshes>,
    pub ribbons: ResMut<'w, GpuRibbonMeshes>,
    #[cfg(feature = "render_3d")]
    pub materials_3d: Option<ResMut<'w, Assets<GpuParticleMaterial3d>>>,
    #[cfg(feature = "render_2d")]
    pub materials_2d: Option<ResMut<'w, Assets<GpuParticleMaterial2d>>>,
}

const CORNERS: [[f32; 2]; 4] = [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]];

/// `capacity` quads, each corner carrying its slot in z.
fn quad_mesh(capacity: u32) -> Mesh {
    let n = capacity.max(1) as usize;
    let mut positions = Vec::with_capacity(n * 4);
    let mut indices = Vec::with_capacity(n * 6);
    for slot in 0..n as u32 {
        let base = slot * 4;
        for c in CORNERS {
            positions.push([c[0], c[1], slot as f32]);
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Until the first readback says where the particles are, a box this big
/// keeps the emitter from being culled.
const UNKNOWN_EXTENT: f32 = 1.0e4;

/// What one GPU emitter's render child is made from, beside its render
/// settings: the kernels and their data, the pool, and its place among the
/// effect's other GPU emitters.
pub struct GpuEmitterSpawn<'a> {
    pub program: &'a GpuProgram,
    pub data: Vec<[f32; 4]>,
    pub capacity: u32,
    pub seed: u64,
    /// The effect's event buffer, or `None` for an emitter in no group,
    /// which gets a buffer of a few bytes so its bind group is whole.
    pub events: Option<Handle<ShaderBuffer>>,
    pub group: Option<Entity>,
    pub order: u32,
    pub inbox: Option<(u32, u32)>,
}

/// The `SORT_*` mode for `render`: the draw order for quads and meshes, the
/// strip order for a `Connect` ribbon.
pub fn sort_mode(render: &RenderSettings, dimension: Dimension) -> u32 {
    if render.draws_ribbon(dimension) {
        return if render.ribbon.kind == RibbonKind::Connect { layout::SORT_CONNECT } else { layout::SORT_NONE };
    }
    match render.draw_order(dimension) {
        DrawOrder::Index => layout::SORT_NONE,
        DrawOrder::OldestFirst => layout::SORT_OLDEST,
        DrawOrder::YoungestFirst => layout::SORT_YOUNGEST,
        DrawOrder::ViewDepth => layout::SORT_DEPTH,
    }
}

/// Spawn a GPU emitter's render child, or `None` when this build or app has
/// no GPU material for `dimension`.
#[cfg_attr(not(all(feature = "render_3d", feature = "render_2d")), allow(unused_variables))]
#[allow(clippy::too_many_arguments)]
pub fn spawn_gpu_renderer(
    commands: &mut Commands,
    assets: &mut GpuRendererAssets,
    meshes: &mut Assets<Mesh>,
    parent: Entity,
    name: &str,
    render: &RenderSettings,
    dimension: Dimension,
    layers: Option<&RenderLayers>,
    spawn: GpuEmitterSpawn,
) -> Option<Entity> {
    let GpuEmitterSpawn { program, data, capacity, seed, events, group, order, inbox } = spawn;
    let capacity = capacity.clamp(1, layout::MAX_GPU_CAPACITY);
    let shader = match assets.shader_cache.0.get(&program.hash) {
        Some(h) => h.clone(),
        None => {
            let path = format!("renzora_particles/gpu/{:016x}.wgsl", program.hash);
            let h = assets.shaders.add(renzora::shader_source::shader(program.source.clone(), path));
            assets.shader_cache.0.insert(program.hash, h.clone());
            h
        }
    };
    // Mesh particles draw a pool mesh of mesh copies, built once the mesh's
    // template exists (`render::mesh_particles`); until then a placeholder
    // with the same vertex layout, so the pipeline is right from the start.
    let draws_mesh = render.draws_mesh(dimension);
    let ribbon = render.draws_ribbon(dimension).then(|| RibbonMesh::for_render(render, dimension, capacity));
    let mesh = if draws_mesh {
        meshes.add(crate::render::mesh_particles::gpu_placeholder_mesh())
    } else if let Some(shape) = ribbon {
        let capped = shape.capped();
        if capped.strips < shape.strips {
            renzora::console_log::console_warn(
                "Particles",
                format!(
                    "emitter `{name}`: its ribbon pool mesh would pass {} M vertices, so the GPU draws {} of its {} strips",
                    MAX_GPU_RIBBON_VERTICES >> 20,
                    capped.strips,
                    shape.strips
                ),
            );
        }
        match assets.ribbons.0.get(&capped) {
            Some(m) => m.clone(),
            None => {
                let m = meshes.add(capped.build());
                assets.ribbons.0.insert(capped, m.clone());
                m
            }
        }
    } else {
        match assets.quads.0.get(&capacity) {
            Some(m) => m.clone(),
            None => {
                let m = meshes.add(quad_mesh(capacity));
                assets.quads.0.insert(capacity, m.clone());
                m
            }
        }
    };
    let trail_points = match render.ribbon.kind {
        RibbonKind::Trail if ribbon.is_some() => render.ribbon.trail_points() as u32,
        _ => 0,
    };
    let particles = assets.buffers.add(ShaderBuffer::with_size(layout::particle_bytes(capacity), RenderAssetUsages::RENDER_WORLD));
    let state = assets.buffers.add(ShaderBuffer::with_size(
        layout::state_bytes_with_trail(capacity, trail_points),
        RenderAssetUsages::RENDER_WORLD,
    ));
    let events = events.unwrap_or_else(|| {
        assets.buffers.add(ShaderBuffer::with_size(u64::from(layout::EVENT_HEADER_WORDS) * 4, RenderAssetUsages::RENDER_WORLD))
    });
    let texture = render
        .texture
        .as_deref()
        .filter(|p| !p.trim().is_empty())
        .map(|p| assets.asset_server.load::<Image>(p.to_string()));
    let sort_mode = sort_mode(render, dimension);
    let mut params = ParticleMaterialParams::for_dimension(render, texture.is_some(), dimension);
    let sorted_quads = matches!(sort_mode, layout::SORT_DEPTH | layout::SORT_OLDEST | layout::SORT_YOUNGEST);
    // What the shader's ribbon fetch reads (see `RibbonMesh` and the GPU
    // FETCH block): the trail history and its length, the taper, and for a
    // Connect ribbon where the sort keys (whose top byte is the strip) start.
    // The fixed axis and the shape's last lanes mean nothing to a ribbon, so
    // they carry these instead of the uniform growing for one facing.
    params.gpu = Vec4::new(
        if sorted_quads { 1.0 } else { 0.0 },
        layout::sorted_word(capacity) as f32,
        layout::trail_word(capacity) as f32,
        trail_points as f32,
    );
    if ribbon.is_some() {
        let finite = |v: f32| if v.is_finite() { v } else { 0.0 };
        params.shape.z = finite(render.ribbon.tail_width);
        params.shape.w = finite(render.ribbon.tail_alpha);
        params.axis = Vec4::new(0.0, 0.0, 0.0, (layout::sorted_word(capacity) - layout::sort_size(capacity)) as f32);
    }

    let link = GpuEmitterLink {
        particles: particles.clone(),
        state: state.clone(),
        shader,
        capacity,
        seed,
        sort_mode,
        ribbon_groups: render.ribbon.groups.clamp(1, layout::MAX_GPU_RIBBON_GROUPS),
        events,
        group,
        order,
        inbox,
        data: Arc::new(data),
        frame: 0,
        reset: false,
        serial: 0,
        steps: Vec::new(),
        frame_input: FrameInput::default(),
        camera: Vec3::ZERO,
        to_mesh: Affine3A::IDENTITY,
        // The readback's radius is a quad's (half the size's diagonal). A
        // unit mesh copy reaches up to its own radius times the largest
        // size, which twice the quad's covers for every built-in shape.
        bounds_pad: if draws_mesh { 2.0 } else { 1.0 },
    };
    let common = (
        Name::new(format!("vfx gpu emitter {name}")),
        renzora::HideInHierarchy,
        renzora::NoSelectionBounds,
        ChildOf(parent),
        Visibility::Inherited,
        bevy::camera::visibility::NoAutoAabb,
        Aabb::from_min_max(Vec3::splat(-UNKNOWN_EXTENT), Vec3::splat(UNKNOWN_EXTENT)),
        link,
        GpuReadoutCell::default(),
        Readback::buffer_range(state.clone(), 0, u64::from(layout::READBACK_WORDS) * 4),
    );
    let entity = match dimension {
        Dimension::ThreeD => {
            #[cfg(feature = "render_3d")]
            {
                let wears_material = cfg!(feature = "material_graph") && render.material_path(dimension).is_some();
                if !wears_material && assets.materials_3d.is_none() {
                    return None;
                }
                let entity = commands
                    .spawn((common, Mesh3d(mesh), crate::render::ParticleMotion::default(), Transform::default()))
                    .id();
                // A project material goes through the engine's resolver and
                // is adopted by `render::surface`, which binds this pool to
                // the material's own fragment shader.
                match render.material_path(dimension) {
                    #[cfg(feature = "material_graph")]
                    Some(path) => {
                        let pool = crate::render::surface::PoolBuffers { particles, state };
                        let slot = crate::render::surface::ParticleSurfaceSlot::new(
                            params,
                            render.blend,
                            render.alpha_cutoff,
                            draws_mesh,
                            Some(pool),
                            texture,
                        )
                        .with_route(crate::render::material::RenderRoute::of(render));
                        commands.entity(entity).insert((renzora::MaterialRef(path.to_string()), slot));
                    }
                    _ => {
                        crate::render::warn_no_material_graph(render, dimension);
                        if let Some(materials) = assets.materials_3d.as_mut() {
                            let material = materials.add(GpuParticleMaterial3d {
                                params,
                                texture,
                                particles,
                                state,
                                blend: render.blend,
                                cutoff: render.alpha_cutoff,
                                mesh: draws_mesh,
                                route: crate::render::material::RenderRoute::of(render),
                            });
                            commands.entity(entity).insert(MeshMaterial3d(material));
                        }
                    }
                }
                if !render.casts_shadows(dimension) {
                    commands.entity(entity).insert(bevy::light::NotShadowCaster);
                }
                if let Some(shape) = render.mesh.clone().filter(|_| draws_mesh) {
                    commands.entity(entity).insert(crate::render::MeshParticles::new(shape, &assets.asset_server, Some(capacity)));
                }
                entity
            }
            #[cfg(not(feature = "render_3d"))]
            return None;
        }
        Dimension::TwoD => {
            #[cfg(feature = "render_2d")]
            {
                let materials = assets.materials_2d.as_mut()?;
                let material = materials.add(GpuParticleMaterial2d {
                    params,
                    texture,
                    particles,
                    state,
                    blend: render.blend,
                    cutoff: render.alpha_cutoff,
                });
                commands
                    .spawn((common, Mesh2d(mesh), MeshMaterial2d(material), Transform::from_xyz(0.0, 0.0, render.z_order)))
                    .id()
            }
            #[cfg(not(feature = "render_2d"))]
            return None;
        }
    };
    if let Some(layers) = layers {
        commands.entity(entity).insert(layers.clone());
    }
    // Sizes in pixels have no world-space box to cull by, and a trail's
    // history reaches back past the live particles the readback bounds,
    // by however far they have travelled, which nothing reads back.
    if render.screen_space_size || trail_points > 0 {
        commands.entity(entity).insert(bevy::camera::visibility::NoFrustumCulling);
    }
    Some(entity)
}

/// Give every GPU emitter that has no render child yet its GPU renderer.
///
/// `sync_instances` leaves a GPU emitter's renderer slot empty for this to
/// fill, because generating the kernels and allocating the pool are the GPU
/// backend's business. An emitter whose kernels cannot be generated, or
/// whose dimension has no GPU material in this app, goes back to the CPU
/// with a Console line saying so, and gets the CPU renderer instead; so do
/// the emitters it shares sub-emitter events with, which cannot step on the
/// GPU without it ([`super::plan`]).
pub fn attach_gpu_renderers(
    mut commands: Commands,
    mut assets: GpuRendererAssets,
    mut cpu: RendererAssets,
    vfx: Res<Assets<crate::asset::VfxAsset>>,
    mut effects: Query<(Entity, &mut ParticleRuntime, Option<&RenderLayers>)>,
) {
    let no_material = || "this app has no GPU particle material for its dimension".to_string();
    for (entity, mut runtime, layers) in &mut effects {
        let runtime = &mut *runtime;
        let Some(asset) = vfx.get(runtime.asset_id) else { continue };
        let pending: Vec<usize> = (0..runtime.sim.emitters.len())
            .filter(|&s| runtime.renderers.get(s).copied().flatten().is_none() && runtime.sim.emitters[s].is_gpu())
            .collect();
        if pending.is_empty() {
            continue;
        }
        let plan = super::plan::plan_effect(&runtime.sim, asset);
        let edges = super::plan::gpu_edges(&runtime.sim);
        let has_material = match runtime.dimension {
            Dimension::ThreeD => cfg!(feature = "render_3d") && gpu_material_3d(&assets),
            Dimension::TwoD => cfg!(feature = "render_2d") && gpu_material_2d(&assets),
        };

        // Every pending emitter's kernels first: one that cannot have them
        // takes its whole group to the CPU before any of it is spawned.
        let mut programs: Vec<Option<Result<GpuProgram, String>>> = vec![None; runtime.sim.emitters.len()];
        let mut to_cpu: Vec<(usize, String)> = Vec::new();
        for &slot in &pending {
            let e = &runtime.sim.emitters[slot];
            let Some(config) = plan.configs[slot].as_ref() else { continue };
            let program = generate_with(&e.compiled, config).map_err(|err| err.to_string());
            let program = match program {
                Ok(_) if !has_material => Err(no_material()),
                other => other,
            };
            if let Err(why) = &program {
                to_cpu.push((slot, why.clone()));
                if plan.linked[slot] {
                    let name = &asset.emitters[e.emitter_index].name;
                    for other in plan.component(slot, &edges).into_iter().filter(|&o| o != slot) {
                        to_cpu.push((other, format!("it shares sub-emitter events with `{name}`, which runs on the CPU")));
                    }
                }
            }
            programs[slot] = Some(program);
        }
        let mut falls: Vec<bool> = vec![false; runtime.sim.emitters.len()];
        for (slot, why) in to_cpu {
            if std::mem::replace(&mut falls[slot], true) {
                continue;
            }
            let e = &mut runtime.sim.emitters[slot];
            let Some(emitter) = asset.emitters.get(e.emitter_index) else { continue };
            renzora::console_log::console_warn("Particles", format!("emitter `{}` runs on the CPU: {why}", emitter.name));
            e.set_gpu(false);
            e.backend = crate::gpu::BackendChoice { active: crate::gpu::ActiveBackend::Cpu, reason: Some(why), fell_back: true };
            // A group member already drawn by the GPU loses that child too.
            if let Some(Some(old)) = runtime.renderers.get(slot).copied() {
                commands.entity(old).try_despawn();
            }
            runtime.renderers[slot] = spawn_renderer(&mut commands, &mut cpu, entity, emitter, runtime.dimension, layers);
        }

        // The effect's one event buffer, shared by every linked GPU emitter.
        let events = plan
            .has_events()
            .then(|| assets.buffers.add(ShaderBuffer::with_size(plan.events.bytes(), RenderAssetUsages::RENDER_WORLD)));
        for &slot in &pending {
            if falls[slot] {
                continue;
            }
            let Some(Ok(program)) = programs[slot].take() else { continue };
            let e = &runtime.sim.emitters[slot];
            let Some(emitter) = asset.emitters.get(e.emitter_index) else { continue };
            let data = layout::pack_data(&e.compiled, &program.consts);
            let linked = plan.linked[slot];
            let spawn = GpuEmitterSpawn {
                program: &program,
                data,
                capacity: e.capacity,
                seed: e.seed,
                events: events.clone().filter(|_| linked),
                group: linked.then_some(entity),
                order: plan.order.get(slot).copied().unwrap_or(0),
                inbox: plan.events.inboxes.get(slot).copied().flatten(),
            };
            let render = runtime.render[slot].clone();
            runtime.renderers[slot] = spawn_gpu_renderer(
                &mut commands,
                &mut assets,
                &mut cpu.meshes,
                entity,
                &emitter.name,
                &render,
                runtime.dimension,
                layers,
                spawn,
            );
        }
    }
}

#[cfg(feature = "render_3d")]
fn gpu_material_3d(assets: &GpuRendererAssets) -> bool {
    assets.materials_3d.is_some()
}

#[cfg(not(feature = "render_3d"))]
fn gpu_material_3d(_assets: &GpuRendererAssets) -> bool {
    false
}

#[cfg(feature = "render_2d")]
fn gpu_material_2d(assets: &GpuRendererAssets) -> bool {
    assets.materials_2d.is_some()
}

#[cfg(not(feature = "render_2d"))]
fn gpu_material_2d(_assets: &GpuRendererAssets) -> bool {
    false
}

/// Copy each GPU emitter's newest readback into its simulation, where the
/// stats, the light and the one-shot rule read it.
pub fn apply_gpu_readouts(mut effects: Query<&mut ParticleRuntime>, cells: Query<&GpuReadoutCell>) {
    for mut runtime in &mut effects {
        let runtime = &mut *runtime;
        for (slot, e) in runtime.sim.emitters.iter_mut().enumerate() {
            if !e.is_gpu() {
                continue;
            }
            let Some(Some(renderer)) = runtime.renderers.get(slot) else { continue };
            if let Ok(GpuReadoutCell(Some(readout))) = cells.get(*renderer) {
                e.apply_readout(*readout);
            }
        }
    }
}

/// Decode a state buffer readback onto its render child.
pub fn on_readback(event: On<ReadbackComplete>, mut cells: Query<&mut GpuReadoutCell>) {
    let Ok(mut cell) = cells.get_mut(event.entity) else { return };
    let words: Vec<u32> = event.data.chunks_exact(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    if let Some(readout) = GpuReadout::decode(&words) {
        cell.0 = Some(readout);
    }
}

/// Hand each GPU emitter's planned steps to the render world, with this
/// frame's transforms and sorting camera, and size its bounds from the last
/// readback.
pub fn sync_gpu_emitters(
    mut counter: ResMut<GpuFrameCounter>,
    mut effects: Query<(&mut ParticleRuntime, &GlobalTransform, Option<&RenderLayers>)>,
    mut links: Query<(&mut GpuEmitterLink, &Transform, &mut Aabb)>,
    cameras: Query<(&Camera, &GlobalTransform, Option<&RenderLayers>), With<Camera3d>>,
    wind: Option<Res<renzora::wind::WindState>>,
) {
    counter.0 = counter.0.wrapping_add(1);
    let frame_index = counter.0;
    let wind = wind.map(|w| *w).unwrap_or_default();
    for (mut runtime, global, layers) in &mut effects {
        let runtime = &mut *runtime;
        let eye_world = sort_eye(cameras.iter(), layers);
        for (slot, e) in runtime.sim.emitters.iter_mut().enumerate() {
            let space = e.space;
            let Some(g) = e.gpu.as_mut() else { continue };
            let Some(Some(renderer)) = runtime.renderers.get(slot) else { continue };
            let Ok((mut link, local, mut aabb)) = links.get_mut(*renderer) else { continue };
            link.frame = frame_index;
            link.reset = std::mem::take(&mut g.reset);
            link.serial = g.serial;
            link.steps.clear();
            link.steps.append(&mut g.plans);
            link.frame_input = FrameInput::at(global.affine()).with_wind(&wind);
            link.to_mesh = sim_to_mesh(space, global, local).unwrap_or(Affine3A::IDENTITY);
            let sim_from_world = match space {
                SimulationSpace::World => Affine3A::IDENTITY,
                SimulationSpace::Local => global.affine().inverse(),
            };
            link.camera = eye_world
                .map(|eye| sim_from_world.transform_point3(eye))
                .filter(|eye| eye.is_finite())
                .unwrap_or(Vec3::ZERO);
            if let Some(next) = gpu_aabb(&g.readout, link.to_mesh, link.bounds_pad) {
                if *aabb != next {
                    *aabb = next;
                }
            }
        }
    }
}

/// The render child's bounds from a readback: the live particles' box in
/// mesh space, padded by the largest particle and by how far the fastest one
/// can move in the frames the readback is late. `None` keeps what is there.
fn gpu_aabb(readout: &GpuReadout, to_mesh: Affine3A, bounds_pad: f32) -> Option<Aabb> {
    const LATE: f32 = 0.1;
    let (lo, hi, radius, speed) = readout.bounds?;
    let radius = radius * bounds_pad;
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for k in 0..8 {
        let corner = Vec3::new(
            if k & 1 == 0 { lo.x } else { hi.x },
            if k & 2 == 0 { lo.y } else { hi.y },
            if k & 4 == 0 { lo.z } else { hi.z },
        );
        let p = to_mesh.transform_point3(corner);
        min = min.min(p);
        max = max.max(p);
    }
    let scale = to_mesh.matrix3.x_axis.length().max(to_mesh.matrix3.y_axis.length()).max(to_mesh.matrix3.z_axis.length());
    let pad = (radius + speed * LATE) * scale;
    let (min, max) = (min - Vec3::splat(pad), max + Vec3::splat(pad));
    (min.is_finite() && max.is_finite()).then(|| Aabb::from_min_max(min, max))
}
