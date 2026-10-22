//! Particles wearing a project `.material`, on either backend.
//!
//! The material keeps everything that makes it that material: its compiled
//! fragment shader, texture slots and parameter buffer (or, for a graph
//! simple enough to be one, a plain `StandardMaterial`), and with them full
//! PBR, image-based lighting, fog, shadows received and every pin the graph
//! drives. What it cannot keep is its vertex shader, which knows nothing of
//! particles. So the particle wears an `ExtendedMaterial<StandardMaterial, _>`
//! of its own, [`ParticleGraphMaterial`] (or [`GpuParticleGraphMaterial`] on
//! the GPU backend), whose extension carries the material's slots at the very
//! bindings the generated shader declares (100..118) and adds the particle
//! uniform (119) and, on the GPU, the simulation's pool (120, 121). Its vertex
//! and prepass vertex shaders are `particle_surface.wesl`, which expands
//! quads, ribbons and mesh copies exactly as `particle3d.wesl` does.
//!
//! ## Why an extension of its own
//!
//! A particle mode on the engine's `SurfaceGraphExt` would have been less
//! code, but an extension's bind group layout belongs to its type, not to an
//! instance. The particle uniform and the vertex-stage storage buffers would
//! then sit in the layout of every graph material in the scene, and WebGL2,
//! which has no vertex storage buffers, could not build a single one. The
//! vertex shader is per type too. So the particle side is two types beside it
//! (CPU and GPU, split for the same WebGL2 reason the built-in GPU material is
//! split from the CPU one), and the graph material is left alone.
//!
//! ## How a material gets here
//!
//! Through the engine's own resolver, so precompiled shaders, material
//! instances, the live codegen of a never-saved graph and the material
//! editor's hot reload all behave as they do on a mesh. The render child is
//! spawned with a `MaterialRef` and a [`ParticleSurfaceSlot`]; the resolver
//! attaches whatever the file compiles to, and [`adopt_surface_materials`],
//! the same frame, swaps that for the particle version before the render
//! world extracts anything. Re-resolving (the editor saving the material)
//! attaches a fresh material, which is adopted again; a material edited in
//! place is rebuilt from its asset event. A file that cannot be resolved falls
//! back to the built-in particle look, with a Console line, rather than
//! drawing nothing.
//!
//! ## What the particle settings still decide
//!
//! The emitter's blend wins over the material's alpha mode (the emitter is
//! where the particle is drawn; a rock material is opaque on a level and may
//! be meant to fade on debris), and the emitter decides shadows, sorting,
//! facing, sizes and the sprite-sheet cell, which reaches the material as its
//! UV. The particle shader's own look (texture, shading, erosion, soft
//! distance, lighting) is the material's business instead; a graph can fade
//! softly with its Depth Fade node. The depth prepass and shadows are the
//! material's base approximation, as on a mesh.
//!
//! ## Deferred
//!
//! A depth-written particle (`Opaque`, `AlphaMask`, `AlphaToCoverage`) on a
//! deferred camera writes the G-buffer, as the same material does on a mesh,
//! so it is lit by the deferred lighting pass and seen by SSR and SSAO. Bevy
//! builds that pass from its prepass pipeline with `DEFERRED_PREPASS` set and
//! the material's `deferred_vertex_shader`, which both extensions point at
//! `particle_surface.wesl`: its `PREPASS_PIPELINE` entry point already
//! expands quads, meshes, ribbons and tubes for the depth prepass and shadows,
//! and the G-buffer pass needs nothing more from it than the particle's own
//! locations 10 and 11, which it writes there too. Left at the default, the
//! extension would inherit `StandardMaterial`'s, Bevy's mesh prepass shader,
//! which reads a particle's centre as a vertex position.
//!
//! The fragment is the graph's own when its compiled shader has a deferred
//! branch (`SurfaceGraphExt::writes_gbuffer`), exactly the test the graph
//! material makes on a mesh, so a node graph reads its Particle nodes in the
//! G-buffer pass as it does forward. A graph without one (it reads the scene
//! depth, normals, motion, transmission or environment map, none of which
//! exist in that pass), and a material that resolved to a plain
//! `StandardMaterial`, write the base approximation through
//! `StandardMaterial`'s own `pbr.wesl`, which reads the prepass vertex output
//! the particle shader writes.
//!
//! Which route a particle takes is the emitter's `RenderMethod` through
//! [`RenderRoute`]: `Auto` by default, the project's choice, so a forward
//! camera draws exactly as before. Transparent blends always draw forward
//! (see `render::material`'s module docs).

use std::marker::PhantomData;

use bevy::asset::uuid::Uuid;
use bevy::ecs::message::{MessageCursor, Messages};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::{ShaderDefVal, ShaderRef};
use renzora::core::MaterialResolved;
use renzora_shader::material::surface_ext::{GraphMaterial, SurfaceGraphExt, SurfaceGraphParams};

use super::material::{
    alpha_mode_3d, is_prepass, vertex_layout, ParticleMaterial3d, ParticleMaterialParams, ParticleRouteMut, RenderRoute,
    MESH_PARTICLES_DEF,
};
use super::ParticleMotion;
use crate::asset::{BlendMode, RenderMethod};

pub const SURFACE_SHADER: &str = "embedded://renzora_particles/shaders/particle_surface.wesl";

/// Set on the fragment stage of every particle-surface pipeline. A graph that
/// reads a Particle node declares the particle's own vertex outputs (locations
/// 10 and 11) only under it, so the same compiled `.material` builds on a mesh,
/// where nothing writes those locations.
pub const PARTICLE_SURFACE_DEF: &str = "RENZORA_PARTICLE_SURFACE";

/// What the pipeline depends on: the material's compiled shader, whether it
/// can write the G-buffer, the vertex layout (quads or mesh copies) and the
/// render route (for the reason `Particle3dKey` carries it). `Uuid`, `bool`
/// and a fieldless enum so the key stays `Copy`, for the reason
/// `SurfaceGraphExtKey` gives.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct ParticleSurfaceKey {
    pub shader_uuid: Option<Uuid>,
    pub writes_gbuffer: bool,
    pub mesh: bool,
    pub method: RenderMethod,
}

/// The CPU backend's extension: the graph material's slots, at its bindings,
/// and the particle uniform.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
#[bind_group_data(ParticleSurfaceKey)]
pub struct ParticleSurfaceExt {
    #[texture(100)]
    #[sampler(101)]
    pub texture_0: Option<Handle<Image>>,
    #[texture(102)]
    pub texture_1: Option<Handle<Image>>,
    #[texture(104)]
    pub texture_2: Option<Handle<Image>>,
    #[texture(106)]
    pub texture_3: Option<Handle<Image>>,
    #[texture(114)]
    pub texture_4: Option<Handle<Image>>,
    #[texture(116)]
    pub texture_5: Option<Handle<Image>>,
    #[texture(108, dimension = "cube")]
    pub cube_0: Option<Handle<Image>>,
    #[texture(110, dimension = "2d_array")]
    pub array_0: Option<Handle<Image>>,
    #[texture(112, dimension = "3d")]
    pub volume_0: Option<Handle<Image>>,
    #[uniform(118)]
    pub params: SurfaceGraphParams,
    #[uniform(119)]
    pub particle: ParticleMaterialParams,
    pub shader_uuid: Option<Uuid>,
    /// The graph's `writes_gbuffer`: its shader has a deferred branch.
    pub writes_gbuffer: bool,
    pub mesh: bool,
    /// Forward or the G-buffer. Not reflected: a route is resolved from the
    /// cameras at run time, never edited or saved.
    #[reflect(ignore)]
    pub route: RenderRoute,
}

/// The GPU backend's extension: [`ParticleSurfaceExt`] plus the pool the
/// vertex shader fetches from.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
#[bind_group_data(ParticleSurfaceKey)]
pub struct GpuParticleSurfaceExt {
    #[texture(100)]
    #[sampler(101)]
    pub texture_0: Option<Handle<Image>>,
    #[texture(102)]
    pub texture_1: Option<Handle<Image>>,
    #[texture(104)]
    pub texture_2: Option<Handle<Image>>,
    #[texture(106)]
    pub texture_3: Option<Handle<Image>>,
    #[texture(114)]
    pub texture_4: Option<Handle<Image>>,
    #[texture(116)]
    pub texture_5: Option<Handle<Image>>,
    #[texture(108, dimension = "cube")]
    pub cube_0: Option<Handle<Image>>,
    #[texture(110, dimension = "2d_array")]
    pub array_0: Option<Handle<Image>>,
    #[texture(112, dimension = "3d")]
    pub volume_0: Option<Handle<Image>>,
    #[uniform(118)]
    pub params: SurfaceGraphParams,
    #[uniform(119)]
    pub particle: ParticleMaterialParams,
    #[storage(120, read_only, visibility(vertex))]
    pub particles: Handle<ShaderBuffer>,
    #[storage(121, read_only, visibility(vertex))]
    pub state: Handle<ShaderBuffer>,
    pub shader_uuid: Option<Uuid>,
    pub writes_gbuffer: bool,
    pub mesh: bool,
    #[reflect(ignore)]
    pub route: RenderRoute,
}

impl From<&ParticleSurfaceExt> for ParticleSurfaceKey {
    fn from(ext: &ParticleSurfaceExt) -> Self {
        Self { shader_uuid: ext.shader_uuid, writes_gbuffer: ext.writes_gbuffer, mesh: ext.mesh, method: ext.route.method }
    }
}

impl From<&GpuParticleSurfaceExt> for ParticleSurfaceKey {
    fn from(ext: &GpuParticleSurfaceExt) -> Self {
        Self { shader_uuid: ext.shader_uuid, writes_gbuffer: ext.writes_gbuffer, mesh: ext.mesh, method: ext.route.method }
    }
}

/// The extended materials keep their route twice: on the extension, for the
/// pipeline key, and as the base's `opaque_render_method`, which is the one
/// Bevy reads (an `ExtendedMaterial` takes its method from its base).
impl ParticleRouteMut for ParticleGraphMaterial {
    fn route(&self) -> RenderRoute {
        self.extension.route
    }
    fn set_route(&mut self, route: RenderRoute) {
        self.extension.route = route;
        self.base.opaque_render_method = route.opaque();
    }
}

impl ParticleRouteMut for GpuParticleGraphMaterial {
    fn route(&self) -> RenderRoute {
        self.extension.route
    }
    fn set_route(&mut self, route: RenderRoute) {
        self.extension.route = route;
        self.base.opaque_render_method = route.opaque();
    }
}

/// A particle wearing a project material, CPU backend.
pub type ParticleGraphMaterial = ExtendedMaterial<StandardMaterial, ParticleSurfaceExt>;
/// A particle wearing a project material, GPU backend.
pub type GpuParticleGraphMaterial = ExtendedMaterial<StandardMaterial, GpuParticleSurfaceExt>;

/// Add `def` to both stages unless it is there already.
fn ensure_def(descriptor: &mut RenderPipelineDescriptor, def: &'static str) {
    let has = |defs: &Vec<ShaderDefVal>| defs.iter().any(|d| matches!(d, ShaderDefVal::Bool(n, true) if n == def));
    if !has(&descriptor.vertex.shader_defs) {
        descriptor.vertex.shader_defs.push(def.into());
    }
    if let Some(fragment) = descriptor.fragment.as_mut() {
        if !has(&fragment.shader_defs) {
            fragment.shader_defs.push(def.into());
        }
    }
}

/// The GPU pool mesh's layout. There is no GPU backend on wasm, so no GPU
/// surface material is ever drawn there and the CPU layout stands in.
#[cfg(not(target_arch = "wasm32"))]
fn pool_layout(
    layout: &MeshVertexBufferLayoutRef,
    mesh: bool,
) -> Result<bevy::mesh::VertexBufferLayout, SpecializedMeshPipelineError> {
    crate::gpu::material::pool_vertex_layout(layout, mesh)
}

#[cfg(target_arch = "wasm32")]
fn pool_layout(
    layout: &MeshVertexBufferLayoutRef,
    mesh: bool,
) -> Result<bevy::mesh::VertexBufferLayout, SpecializedMeshPipelineError> {
    vertex_layout(layout, mesh, true)
}

/// The shared half of both extensions' `specialize`.
fn specialize_surface(
    descriptor: &mut RenderPipelineDescriptor,
    layout: &MeshVertexBufferLayoutRef,
    key: ParticleSurfaceKey,
    gpu: bool,
) -> Result<(), SpecializedMeshPipelineError> {
    let prepass = is_prepass(descriptor);
    descriptor.vertex.buffers = vec![if gpu { pool_layout(layout, key.mesh)? } else { vertex_layout(layout, key.mesh, true)? }];
    // The particle vertex shader writes a UV, a colour and a tangent whatever
    // the particle mesh holds (a GPU pool mesh holds only the slot), so the
    // fragment is told it has them, the way Bevy would for a mesh that did.
    for def in ["VERTEX_UVS", "VERTEX_UVS_A", "VERTEX_COLORS", "VERTEX_TANGENTS", PARTICLE_SURFACE_DEF] {
        ensure_def(descriptor, def);
    }
    if gpu {
        ensure_def(descriptor, "GPU_PARTICLES");
    }
    if key.mesh {
        ensure_def(descriptor, MESH_PARTICLES_DEF);
    }
    // Quads face the view in any winding and meshes are drawn from both
    // sides, as with the particle shader.
    descriptor.primitive.cull_mode = None;
    // The prepass and shadow passes keep StandardMaterial's own fragment (the
    // base approximation), as a graph material does on a mesh; the main pass
    // runs the graph's. So does the G-buffer pass when the graph's shader has
    // a deferred branch, because that pass is all deferred lighting sees: the
    // base there would draw every node graph as its approximation. Without
    // one the shader would not compile in it (other vertex input, other view
    // bindings), so the base stays.
    if !prepass || (key.writes_gbuffer && is_deferred_prepass(descriptor)) {
        if let (Some(uuid), Some(fragment)) = (key.shader_uuid, descriptor.fragment.as_mut()) {
            fragment.shader = Handle::<Shader>::Uuid(uuid, PhantomData);
        }
    }
    Ok(())
}

/// Whether `descriptor` is the deferred G-buffer pass: the prepass pipeline
/// with `DEFERRED_PREPASS` set, which Bevy sets only for a camera with a
/// `DeferredPrepass` drawing a material whose render method is deferred.
pub(crate) fn is_deferred_prepass(descriptor: &RenderPipelineDescriptor) -> bool {
    descriptor
        .fragment
        .as_ref()
        .is_some_and(|f| f.shader_defs.iter().any(|d| matches!(d, ShaderDefVal::Bool(n, true) if n == "DEFERRED_PREPASS")))
}

impl MaterialExtension for ParticleSurfaceExt {
    fn vertex_shader() -> ShaderRef {
        SURFACE_SHADER.into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        SURFACE_SHADER.into()
    }

    // The G-buffer pass places the particle like every other pass; see the
    // module docs for what the default would run instead. The fragment is
    // left to the base (`pbr.wesl`) and swapped for the graph's in
    // `specialize` when it can write the G-buffer.
    fn deferred_vertex_shader() -> ShaderRef {
        SURFACE_SHADER.into()
    }

    fn enable_oit() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        specialize_surface(descriptor, layout, key.bind_group_data, false)
    }
}

impl MaterialExtension for GpuParticleSurfaceExt {
    fn vertex_shader() -> ShaderRef {
        SURFACE_SHADER.into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        SURFACE_SHADER.into()
    }

    fn deferred_vertex_shader() -> ShaderRef {
        SURFACE_SHADER.into()
    }

    fn enable_oit() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        specialize_surface(descriptor, layout, key.bind_group_data, true)
    }
}

/// The GPU pool a [`GpuParticleSurfaceExt`] fetches from.
#[derive(Clone, Debug)]
pub struct PoolBuffers {
    pub particles: Handle<ShaderBuffer>,
    pub state: Handle<ShaderBuffer>,
}

/// What a material was adopted from, so an edit to it can be followed.
#[derive(Clone, Debug, Default)]
enum Source {
    #[default]
    Pending,
    Graph(Handle<GraphMaterial>),
    Standard(Handle<StandardMaterial>),
    /// Resolving failed; drawn with the particle shader.
    Fallback,
}

/// On a render child that wears a project material: the particle side of its
/// material, kept for as long as it lives so a re-resolved or edited material
/// is rebuilt with the same particle settings.
#[derive(Component, Clone, Debug)]
pub struct ParticleSurfaceSlot {
    pub params: ParticleMaterialParams,
    pub blend: BlendMode,
    pub cutoff: f32,
    pub mesh: bool,
    /// `Some` on the GPU backend.
    pub pool: Option<PoolBuffers>,
    /// The emitter's texture, for the built-in look a failed material falls
    /// back to.
    pub texture: Option<Handle<Image>>,
    /// The emitter's render method. A rebuilt material starts from this, and
    /// `resolve_render_routes` settles a deferred request the same frame.
    pub route: RenderRoute,
    source: Source,
}

impl ParticleSurfaceSlot {
    pub fn new(
        params: ParticleMaterialParams,
        blend: BlendMode,
        cutoff: f32,
        mesh: bool,
        pool: Option<PoolBuffers>,
        texture: Option<Handle<Image>>,
    ) -> Self {
        // A transparent blend is forward whatever is asked (see
        // `RenderSettings::effective_render_method`); a solid one follows the
        // project until `with_route` says otherwise.
        let method = if blend.writes_depth() { RenderMethod::Auto } else { RenderMethod::Forward };
        Self { params, blend, cutoff, mesh, pool, texture, route: RenderRoute::new(method), source: Source::Pending }
    }

    /// The same slot with the emitter's route (`RenderRoute::of`).
    pub fn with_route(mut self, route: RenderRoute) -> Self {
        self.route = route;
        self
    }

    /// Whether the resolver's material has been swapped for the particle one.
    pub fn adopted(&self) -> bool {
        !matches!(self.source, Source::Pending)
    }
}

/// The particle version of a resolved material's base: the emitter's blend in
/// place of the material's alpha mode, the emitter's render route in place of
/// the material's method (a rock that is forward on a level may be debris on
/// a deferred camera, and the emitter is where it is drawn), and both faces
/// drawn. A quad's normal already faces the view, so only a mesh copy is
/// double sided (which is what flips a back face's normal toward the eye).
pub fn particle_base(mut base: StandardMaterial, blend: BlendMode, cutoff: f32, mesh: bool, route: RenderRoute) -> StandardMaterial {
    base.alpha_mode = alpha_mode_3d(blend, cutoff);
    base.opaque_render_method = route.opaque();
    base.cull_mode = None;
    base.double_sided = mesh;
    base
}

macro_rules! fill_slots {
    ($ext:expr, $graph:expr) => {{
        let g: &SurfaceGraphExt = $graph;
        $ext.texture_0 = g.texture_0.clone();
        $ext.texture_1 = g.texture_1.clone();
        $ext.texture_2 = g.texture_2.clone();
        $ext.texture_3 = g.texture_3.clone();
        $ext.texture_4 = g.texture_4.clone();
        $ext.texture_5 = g.texture_5.clone();
        $ext.cube_0 = g.cube_0.clone();
        $ext.array_0 = g.array_0.clone();
        $ext.volume_0 = g.volume_0.clone();
        $ext.params = g.params.clone();
        $ext.shader_uuid = g.shader_uuid;
        $ext.writes_gbuffer = g.writes_gbuffer;
    }};
}

/// The CPU material for `slot` from a resolved base and, for a graph, its
/// extension. Unset slots stay `None`, which Bevy binds its fallback image to.
pub fn cpu_material(slot: &ParticleSurfaceSlot, base: StandardMaterial, graph: Option<&SurfaceGraphExt>) -> ParticleGraphMaterial {
    let mut extension = ParticleSurfaceExt { particle: slot.params, mesh: slot.mesh, route: slot.route, ..Default::default() };
    if let Some(g) = graph {
        fill_slots!(extension, g);
    }
    ExtendedMaterial { base: particle_base(base, slot.blend, slot.cutoff, slot.mesh, slot.route), extension }
}

/// [`cpu_material`] for the GPU backend.
pub fn gpu_material(
    slot: &ParticleSurfaceSlot,
    pool: &PoolBuffers,
    base: StandardMaterial,
    graph: Option<&SurfaceGraphExt>,
) -> GpuParticleGraphMaterial {
    let mut extension = GpuParticleSurfaceExt {
        particle: slot.params,
        mesh: slot.mesh,
        particles: pool.particles.clone(),
        state: pool.state.clone(),
        route: slot.route,
        ..Default::default()
    };
    if let Some(g) = graph {
        fill_slots!(extension, g);
    }
    ExtendedMaterial { base: particle_base(base, slot.blend, slot.cutoff, slot.mesh, slot.route), extension }
}

/// Every material asset the adoption touches.
#[derive(bevy::ecs::system::SystemParam)]
pub struct SurfaceAssets<'w> {
    graphs: Option<Res<'w, Assets<GraphMaterial>>>,
    standards: Option<Res<'w, Assets<StandardMaterial>>>,
    cpu: Option<ResMut<'w, Assets<ParticleGraphMaterial>>>,
    builtin: Option<ResMut<'w, Assets<ParticleMaterial3d>>>,
    #[cfg(not(target_arch = "wasm32"))]
    gpu: Option<ResMut<'w, Assets<GpuParticleGraphMaterial>>>,
    #[cfg(not(target_arch = "wasm32"))]
    gpu_builtin: Option<ResMut<'w, Assets<crate::gpu::material::GpuParticleMaterial3d>>>,
}

/// Swap the material the resolver attached to each particle render child for
/// its particle version (see the module docs), follow edits to the source,
/// and fall back to the built-in look when the file could not be resolved.
#[allow(clippy::type_complexity)]
pub fn adopt_surface_materials(
    mut commands: Commands,
    mut slots: Query<(
        Entity,
        &mut ParticleSurfaceSlot,
        Option<&MeshMaterial3d<GraphMaterial>>,
        Option<&MeshMaterial3d<StandardMaterial>>,
        Has<MaterialResolved>,
        Option<&renzora::MaterialRef>,
        Option<&ParticleMotion>,
    )>,
    mut assets: SurfaceAssets,
    // Read through cursors rather than `MessageReader`s, so an app without
    // the material system's assets (a test, a tool) runs this harmlessly
    // instead of failing on a message type nobody registered.
    graph_events: Option<Res<Messages<AssetEvent<GraphMaterial>>>>,
    standard_events: Option<Res<Messages<AssetEvent<StandardMaterial>>>>,
    mut graph_cursor: Local<MessageCursor<AssetEvent<GraphMaterial>>>,
    mut standard_cursor: Local<MessageCursor<AssetEvent<StandardMaterial>>>,
) {
    fn modified<A: Asset>(events: Option<&Messages<AssetEvent<A>>>, cursor: &mut MessageCursor<AssetEvent<A>>) -> Vec<AssetId<A>> {
        let Some(events) = events else { return Vec::new() };
        cursor
            .read(events)
            .filter_map(|e| match e {
                AssetEvent::Modified { id } => Some(*id),
                _ => None,
            })
            .collect()
    }
    let modified_graphs = modified(graph_events.as_deref(), &mut graph_cursor);
    let modified_standards = modified(standard_events.as_deref(), &mut standard_cursor);
    for (entity, mut slot, graph, standard, resolved, material_ref, motion) in &mut slots {
        // What to build from this frame: a material the resolver just
        // attached, or the one already adopted when its asset changed. A
        // re-resolve that fails leaves whatever was adopted before drawing.
        let source = match (graph, standard) {
            (Some(g), _) => Source::Graph(g.0.clone()),
            (None, Some(s)) => Source::Standard(s.0.clone()),
            (None, None) => match &slot.source {
                Source::Graph(h) if modified_graphs.contains(&h.id()) => Source::Graph(h.clone()),
                Source::Standard(h) if modified_standards.contains(&h.id()) => Source::Standard(h.clone()),
                Source::Pending if resolved => Source::Fallback,
                _ => continue,
            },
        };
        // A rebuild keeps what the motion sync last wrote.
        if let Some(m) = motion {
            slot.params.motion.x = m.rate;
            if let Some(world) = m.world {
                slot.params.motion.y = if world { 1.0 } else { 0.0 };
            }
        }
        let (base, graph_ext) = match &source {
            Source::Graph(h) => match assets.graphs.as_ref().and_then(|a| a.get(h)) {
                Some(g) => (g.base.clone(), Some(g.extension.clone())),
                None => continue,
            },
            Source::Standard(h) => match assets.standards.as_ref().and_then(|a| a.get(h)) {
                Some(s) => (s.clone(), None),
                None => continue,
            },
            Source::Fallback => {
                let path = material_ref.map(|r| r.0.as_str()).unwrap_or("");
                renzora::console_log::console_warn(
                    "Particles",
                    format!("the material `{path}` could not be used on particles; they draw with the particle shader"),
                );
                attach_fallback(&mut commands, entity, &slot, &mut assets);
                slot.source = Source::Fallback;
                continue;
            }
            Source::Pending => continue,
        };
        let mut e = commands.entity(entity);
        e.remove::<(MeshMaterial3d<GraphMaterial>, MeshMaterial3d<StandardMaterial>, MeshMaterial3d<ParticleMaterial3d>)>();
        match slot.pool.clone() {
            None => {
                let Some(cpu) = assets.cpu.as_mut() else { continue };
                let handle = cpu.add(cpu_material(&slot, base, graph_ext.as_ref()));
                e.insert(MeshMaterial3d(handle));
            }
            #[cfg(not(target_arch = "wasm32"))]
            Some(pool) => {
                e.remove::<MeshMaterial3d<crate::gpu::material::GpuParticleMaterial3d>>();
                let Some(gpu) = assets.gpu.as_mut() else { continue };
                let handle = gpu.add(gpu_material(&slot, &pool, base, graph_ext.as_ref()));
                e.insert(MeshMaterial3d(handle));
            }
            #[cfg(target_arch = "wasm32")]
            Some(_) => continue,
        }
        slot.source = source;
    }
}

/// The built-in particle look, for a material that would not resolve. A
/// `.shader` file resolves to a code material with its own vertex stage,
/// which cannot place a particle either, so it is taken off too.
fn attach_fallback(commands: &mut Commands, entity: Entity, slot: &ParticleSurfaceSlot, assets: &mut SurfaceAssets) {
    commands.entity(entity).remove::<MeshMaterial3d<renzora_shader::runtime::CodeShaderMaterial>>();
    match &slot.pool {
        None => {
            let Some(builtin) = assets.builtin.as_mut() else { return };
            let handle = builtin.add(ParticleMaterial3d {
                params: slot.params,
                texture: slot.texture.clone(),
                blend: slot.blend,
                cutoff: slot.cutoff,
                mesh: slot.mesh,
                route: slot.route,
            });
            commands.entity(entity).insert(MeshMaterial3d(handle));
        }
        #[cfg(not(target_arch = "wasm32"))]
        Some(pool) => {
            let Some(builtin) = assets.gpu_builtin.as_mut() else { return };
            let handle = builtin.add(crate::gpu::material::GpuParticleMaterial3d {
                params: slot.params,
                texture: slot.texture.clone(),
                particles: pool.particles.clone(),
                state: pool.state.clone(),
                blend: slot.blend,
                cutoff: slot.cutoff,
                mesh: slot.mesh,
                route: slot.route,
            });
            commands.entity(entity).insert(MeshMaterial3d(handle));
        }
        #[cfg(target_arch = "wasm32")]
        Some(_) => {}
    }
}

impl super::ParticleParamsMut for ParticleGraphMaterial {
    fn particle_params_mut(&mut self) -> &mut ParticleMaterialParams {
        &mut self.extension.particle
    }
}

impl super::ParticleParamsMut for GpuParticleGraphMaterial {
    fn particle_params_mut(&mut self) -> &mut ParticleMaterialParams {
        &mut self.extension.particle
    }
}

/// Register the particle-surface materials, their shader and their systems.
/// The GPU type only where the GPU backend exists, since its layout holds
/// vertex-stage storage buffers.
pub fn plugin(app: &mut App) {
    app.add_plugins(bevy::pbr::MaterialPlugin::<ParticleGraphMaterial>::default());
    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugins(bevy::pbr::MaterialPlugin::<GpuParticleGraphMaterial>::default());
    app.add_systems(
        PostUpdate,
        (
            adopt_surface_materials,
            super::sync_motion_rate::<ParticleGraphMaterial>.after(crate::systems::step_effects),
        )
            .in_set(crate::ParticleSystems),
    );
    app.add_systems(PostUpdate, super::material::resolve_render_routes::<ParticleGraphMaterial>.after(crate::ParticleSystems));
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(
        PostUpdate,
        (
            super::sync_motion_rate::<GpuParticleGraphMaterial>
                .after(crate::systems::step_effects)
                .in_set(crate::ParticleSystems),
            super::material::resolve_render_routes::<GpuParticleGraphMaterial>.after(crate::ParticleSystems),
        ),
    );
}
