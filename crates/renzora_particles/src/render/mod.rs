//! Drawing: one dynamic mesh per emitter, rebuilt in place every step.
//!
//! No render-graph code. Each emitter gets a hidden child entity holding an
//! ordinary `Mesh3d` + [`ParticleMaterial3d`] (or `Mesh2d` +
//! [`ParticleMaterial2d`]), so particles are culled, sorted against the rest of
//! the scene, lit by nothing and drawn by Bevy's own mesh pipelines. Each
//! particle is four vertices that all carry its centre, its corner, its size,
//! rotation, colour, sprite frame, a per-particle seed and its life fraction;
//! the vertex shader does the expansion and the fragment shader the procedural
//! shading (see [`crate::asset::Shading`]). A ribbon emitter writes strips
//! instead of quads into the same mesh, with the same vertex layout and the
//! same material ([`ribbon`]). A mesh-particle emitter writes a copy of its
//! mesh per particle instead, with two more columns, and the vertex shader
//! turns and sizes each copy ([`mesh_particles`]). An emitter wearing a
//! project `.material` writes exactly the same mesh and draws it through the
//! material's own fragment shader instead of the particle one ([`surface`]).
//!
//! ## Shadows and motion
//!
//! A renderer carries `NotShadowCaster` unless its settings say it casts
//! ([`RenderSettings::casts_shadows`]), and a [`ParticleMotion`] holding the
//! playback rate its material's motion vectors last saw, which
//! [`sync_motion_rate`] keeps in step with the effect's playback.
//!
//! ## No per-frame asset churn
//!
//! The mesh asset is created once per emitter and mutated in place: the
//! attribute vectors are taken out of the mesh, cleared, refilled and put back,
//! so their allocations are reused and no handle is ever replaced. A paused
//! effect writes nothing at all.
//!
//! ## Mesh space
//!
//! The renderer is a child of the effect's entity, so the mesh is drawn through
//! the entity's transform. A `Local` emitter's particles are already in that
//! space. A `World` emitter's particles are brought into it with the inverse of
//! the renderer's current world transform, computed after transform
//! propagation in the same frame the GPU will use it, so a moving emitter's
//! trail does not lag by a frame.
//!
//! ## Bounds
//!
//! The vertex positions are particle centres, so the AABB Bevy would derive from
//! them is too small by half a particle. Each renderer carries `NoAutoAabb` and
//! the AABB is written here from the centres padded by each particle's radius.
//!
//! ## Render layers
//!
//! Bevy does not propagate `RenderLayers` down a hierarchy, so a renderer child
//! of an effect on a private layer (the editor's offscreen preview) used to sit
//! on layer 0 and draw into the scene viewport instead of the preview. Each
//! renderer copies its effect's layers when it is spawned.

pub mod material;
pub mod mesh_particles;
pub mod ribbon;
#[cfg(feature = "material_graph")]
pub mod surface;

#[cfg(test)]
mod tests;

use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::ecs::system::SystemParam;
use bevy::math::Affine3A;
use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::*;

use crate::asset::{Dimension, Emitter, Facing, RenderSettings, SheetMode, SimulationSpace};
use crate::component::{ParticlePlayback, PlayState};
use crate::sim::ParticleBuffer;
pub use material::{
    ParticleMaterialParams, ATTRIBUTE_PARTICLE_AXIS, ATTRIBUTE_PARTICLE_EXTRA, ATTRIBUTE_PARTICLE_LOCAL,
    ATTRIBUTE_PARTICLE_PARAMS, ATTRIBUTE_PARTICLE_VELOCITY,
};
pub use mesh_particles::{MeshParticles, MeshTemplate, MeshTemplates};
#[cfg(feature = "render_2d")]
pub use material::ParticleMaterial2d;
#[cfg(feature = "render_3d")]
pub use material::ParticleMaterial3d;

/// On each emitter's render child.
#[derive(Component)]
pub struct ParticleRenderer {
    pub mesh: Handle<Mesh>,
}

/// On each 3D render child: the playback rate its material's uniform holds
/// (`ParticleMaterialParams::motion.x`), so [`sync_motion_rate`] touches the
/// material asset only when the rate actually changes. Writing it every frame
/// would re-prepare every particle bind group every frame.
#[derive(Component, Clone, Copy, Debug)]
pub struct ParticleMotion {
    pub rate: f32,
    /// `motion.y`: the emitter simulates in world space. `None` until the
    /// sync has written it, for a renderer spawned without knowing.
    pub world: Option<bool>,
}

impl Default for ParticleMotion {
    fn default() -> Self {
        Self { rate: 1.0, world: None }
    }
}

impl ParticleMotion {
    /// What a renderer whose uniform was built by `in_space(space)` holds.
    pub fn in_space(space: SimulationSpace) -> Self {
        Self { rate: 1.0, world: Some(space == SimulationSpace::World) }
    }
}

/// Simulated seconds per second of frame: what `step_effects` advances an
/// effect by, over the frame's delta. An editor clock plays at its own rate
/// whatever the saved state says; otherwise it is the time scale while
/// playing and 0 while paused or stopped.
pub fn playback_rate(playback: &ParticlePlayback) -> f32 {
    let rate = match (playback.editor_clock, playback.state) {
        (Some(clock), _) => clock,
        (None, PlayState::Playing) => playback.time_scale,
        (None, _) => 0.0,
    };
    if rate.is_finite() {
        rate.max(0.0)
    } else {
        0.0
    }
}

/// A 3D particle material whose uniform the motion sync can reach.
pub trait ParticleParamsMut {
    fn particle_params_mut(&mut self) -> &mut ParticleMaterialParams;
}

#[cfg(feature = "render_3d")]
impl ParticleParamsMut for ParticleMaterial3d {
    fn particle_params_mut(&mut self) -> &mut ParticleMaterialParams {
        &mut self.params
    }
}

/// Keep each renderer's motion-vector playback rate in step with its effect,
/// for renderers drawn with material `M`. Runs after the step, before the
/// render world extracts, and writes only on a change (pause, resume, a new
/// time scale), so a playing effect costs nothing here.
#[cfg(feature = "render_3d")]
pub fn sync_motion_rate<M: bevy::pbr::Material + ParticleParamsMut>(
    effects: Query<(&crate::systems::ParticleRuntime, &ParticlePlayback)>,
    mut renderers: Query<(&MeshMaterial3d<M>, &mut ParticleMotion)>,
    mut materials: ResMut<Assets<M>>,
) {
    for (runtime, playback) in &effects {
        let rate = playback_rate(playback);
        for (slot, renderer) in runtime.renderers.iter().enumerate() {
            let Some(renderer) = renderer else { continue };
            let Ok((material, mut motion)) = renderers.get_mut(*renderer) else { continue };
            let world = runtime.sim.emitters.get(slot).map(|e| e.space == SimulationSpace::World);
            if motion.rate == rate && (world.is_none() || motion.world == world) {
                continue;
            }
            if let Some(mut m) = materials.get_mut(&material.0) {
                let params = m.particle_params_mut();
                params.motion.x = rate;
                if let Some(world) = world {
                    params.motion.y = if world { 1.0 } else { 0.0 };
                }
                motion.rate = rate;
                motion.world = world.or(motion.world);
            }
        }
    }
}

/// The assets a renderer needs, with each material optional so the runtime
/// works in a build or an app that has only one of the two renderers.
#[derive(SystemParam)]
pub struct RendererAssets<'w> {
    pub meshes: ResMut<'w, Assets<Mesh>>,
    pub asset_server: Res<'w, AssetServer>,
    #[cfg(feature = "render_3d")]
    pub materials_3d: Option<ResMut<'w, Assets<ParticleMaterial3d>>>,
    #[cfg(feature = "render_2d")]
    pub materials_2d: Option<ResMut<'w, Assets<ParticleMaterial2d>>>,
}

/// A mesh with one degenerate quad and every attribute present, so the
/// pipeline's vertex layout always resolves and the mesh is never empty.
/// `mesh_mode` adds the two attributes the mesh-particle layout reads.
#[cfg(any(feature = "render_3d", feature = "render_2d"))]
fn new_mesh(mesh_mode: bool) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    let mut mesh = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    let mut scratch = MeshScratch::default();
    if mesh_mode {
        scratch.push_degenerate_mesh();
    } else {
        scratch.push_degenerate();
    }
    scratch.store(&mut mesh);
    mesh
}

/// Spawn the render child for one emitter. `None` when this build or app has
/// no renderer for `dimension`. `layers` is the effect entity's own
/// `RenderLayers`, copied onto the child (see the module docs).
#[cfg_attr(
    not(all(feature = "render_3d", feature = "render_2d")),
    allow(unused_variables)
)]
pub fn spawn_renderer(
    commands: &mut Commands,
    assets: &mut RendererAssets,
    parent: Entity,
    emitter: &Emitter,
    dimension: Dimension,
    layers: Option<&RenderLayers>,
) -> Option<Entity> {
    let entity = spawn_renderer_inner(commands, assets, parent, emitter, dimension)?;
    if let Some(layers) = layers {
        commands.entity(entity).insert(layers.clone());
    }
    Some(entity)
}

#[cfg_attr(
    not(all(feature = "render_3d", feature = "render_2d")),
    allow(unused_variables)
)]
fn spawn_renderer_inner(
    commands: &mut Commands,
    assets: &mut RendererAssets,
    parent: Entity,
    emitter: &Emitter,
    dimension: Dimension,
) -> Option<Entity> {
    match dimension {
        Dimension::ThreeD => {
            #[cfg(feature = "render_3d")]
            {
                let render = &emitter.render;
                let (params, texture) = material_inputs(&assets.asset_server, render, dimension);
                let params = params.in_space(emitter.space);
                let draws_mesh = render.draws_mesh(dimension);
                // A project material is worn through `MaterialRef`, so the
                // engine's resolver compiles it like any other; `surface`
                // swaps what it attaches for the particle version. The mesh
                // is the particle shader's either way.
                let material = match render.material_path(dimension) {
                    #[cfg(feature = "material_graph")]
                    Some(path) => Some(RendererMaterial::Surface(
                        renzora::MaterialRef(path.to_string()),
                        Box::new(surface::ParticleSurfaceSlot::new(
                            params,
                            render.blend,
                            render.alpha_cutoff,
                            draws_mesh,
                            None,
                            texture.clone(),
                        )
                        .with_route(material::RenderRoute::of(render))),
                    )),
                    _ => {
                        warn_no_material_graph(render, dimension);
                        assets.materials_3d.as_mut().map(|materials| {
                            RendererMaterial::Builtin(materials.add(ParticleMaterial3d {
                                params,
                                texture,
                                blend: render.blend,
                                cutoff: render.alpha_cutoff,
                                mesh: draws_mesh,
                                route: material::RenderRoute::of(render),
                            }))
                        })
                    }
                };
                if let Some(material) = material {
                    let mesh = assets.meshes.add(new_mesh(draws_mesh));
                    let entity = commands
                        .spawn((
                            Mesh3d(mesh.clone()),
                            ParticleRenderer { mesh },
                            ParticleMotion::in_space(emitter.space),
                            renderer_bundle(emitter, parent, 0.0),
                        ))
                        .id();
                    match material {
                        RendererMaterial::Builtin(handle) => {
                            commands.entity(entity).insert(MeshMaterial3d(handle));
                        }
                        #[cfg(feature = "material_graph")]
                        RendererMaterial::Surface(material_ref, slot) => {
                            commands.entity(entity).insert((material_ref, *slot));
                        }
                    }
                    if !render.casts_shadows(dimension) {
                        commands.entity(entity).insert(bevy::light::NotShadowCaster);
                    }
                    if let Some(shape) = render.mesh.clone().filter(|_| draws_mesh) {
                        commands.entity(entity).insert(MeshParticles::new(shape, &assets.asset_server, None));
                    }
                    // A size in pixels is any size in world units, so no box
                    // bounds it: the emitter is never frustum culled.
                    if render.screen_space_size {
                        commands.entity(entity).insert(bevy::camera::visibility::NoFrustumCulling);
                    }
                    return Some(entity);
                }
            }
            None
        }
        Dimension::TwoD => {
            #[cfg(feature = "render_2d")]
            if let Some(materials) = assets.materials_2d.as_mut() {
                let (params, texture) = material_inputs(&assets.asset_server, &emitter.render, dimension);
                let material = materials.add(ParticleMaterial2d {
                    params,
                    texture,
                    blend: emitter.render.blend,
                    cutoff: emitter.render.alpha_cutoff,
                });
                let mesh = assets.meshes.add(new_mesh(false));
                let entity = commands
                    .spawn((
                        Mesh2d(mesh.clone()),
                        MeshMaterial2d(material),
                        ParticleRenderer { mesh },
                        renderer_bundle(emitter, parent, emitter.render.z_order),
                    ))
                    .id();
                if emitter.render.screen_space_size {
                    commands.entity(entity).insert(bevy::camera::visibility::NoFrustumCulling);
                }
                return Some(entity);
            }
            None
        }
    }
}

/// What a 3D renderer is drawn with.
#[cfg(feature = "render_3d")]
enum RendererMaterial {
    Builtin(Handle<ParticleMaterial3d>),
    #[cfg(feature = "material_graph")]
    Surface(renzora::MaterialRef, Box<surface::ParticleSurfaceSlot>),
}

/// A build without the material system draws a material-wearing emitter with
/// the particle shader, and says so once, rather than drawing nothing.
#[cfg(feature = "render_3d")]
pub(crate) fn warn_no_material_graph(render: &RenderSettings, dimension: Dimension) {
    if cfg!(feature = "material_graph") {
        return;
    }
    if let Some(path) = render.material_path(dimension) {
        static WARNED: std::sync::Once = std::sync::Once::new();
        let path = path.to_string();
        WARNED.call_once(|| {
            renzora::console_log::console_warn(
                "Particles",
                format!("this build has no material system, so `{path}` on particles draws with the particle shader"),
            );
        });
    }
}

/// The uniform and the optional texture for an emitter's material.
#[cfg(any(feature = "render_3d", feature = "render_2d"))]
pub(crate) fn material_inputs(
    asset_server: &AssetServer,
    render: &RenderSettings,
    dimension: Dimension,
) -> (ParticleMaterialParams, Option<Handle<Image>>) {
    let texture = render
        .texture
        .as_deref()
        .filter(|p| !p.trim().is_empty())
        .map(|p| asset_server.load::<Image>(p.to_string()));
    let params = ParticleMaterialParams::for_dimension(render, texture.is_some(), dimension);
    (params, texture)
}

/// What every renderer child carries, whichever renderer draws it.
#[cfg(any(feature = "render_3d", feature = "render_2d"))]
fn renderer_bundle(emitter: &Emitter, parent: Entity, z: f32) -> impl Bundle {
    (
        Name::new(format!("vfx emitter {}", emitter.name)),
        // Editor plumbing, not scene content: hidden from the hierarchy, and
        // therefore never written into a saved scene either.
        renzora::HideInHierarchy,
        // Its bounds follow wherever the particles are this frame, so a
        // selection box taken from it jumped with the effect. The editor's
        // particle gizmo marks the emitter instead.
        renzora::NoSelectionBounds,
        ChildOf(parent),
        Transform::from_xyz(0.0, 0.0, z),
        Visibility::Hidden,
        bevy::camera::visibility::NoAutoAabb,
        Aabb::default(),
    )
}

/// Reusable vertex columns.
#[derive(Default)]
struct MeshScratch {
    position: Vec<[f32; 3]>,
    corner: Vec<[f32; 2]>,
    color: Vec<[f32; 4]>,
    params: Vec<[f32; 4]>,
    axis: Vec<[f32; 3]>,
    extra: Vec<[f32; 4]>,
    /// Mesh particles only: each vertex's position and normal in the source
    /// mesh. Empty for quads and ribbons, and then not stored at all.
    local: Vec<[f32; 3]>,
    normal: Vec<[f32; 3]>,
    /// Each vertex's particle velocity in mesh space. Padded with zeros up to
    /// the vertex count when stored, so a writer that knows nothing of it (a
    /// ribbon strip) still leaves a mesh every column of which is complete.
    velocity: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

const CORNERS: [[f32; 2]; 4] = [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]];

impl MeshScratch {
    /// Take the columns out of `mesh` so their allocations are reused.
    fn take(mesh: &mut Mesh) -> Self {
        fn f3(v: Option<VertexAttributeValues>) -> Vec<[f32; 3]> {
            match v {
                Some(VertexAttributeValues::Float32x3(mut v)) => {
                    v.clear();
                    v
                }
                _ => Vec::new(),
            }
        }
        fn f4(v: Option<VertexAttributeValues>) -> Vec<[f32; 4]> {
            match v {
                Some(VertexAttributeValues::Float32x4(mut v)) => {
                    v.clear();
                    v
                }
                _ => Vec::new(),
            }
        }
        let corner = match mesh.remove_attribute(Mesh::ATTRIBUTE_UV_0) {
            Some(VertexAttributeValues::Float32x2(mut v)) => {
                v.clear();
                v
            }
            _ => Vec::new(),
        };
        let indices = match mesh.remove_indices() {
            Some(Indices::U32(mut v)) => {
                v.clear();
                v
            }
            _ => Vec::new(),
        };
        Self {
            position: f3(mesh.remove_attribute(Mesh::ATTRIBUTE_POSITION)),
            corner,
            color: f4(mesh.remove_attribute(Mesh::ATTRIBUTE_COLOR)),
            params: f4(mesh.remove_attribute(ATTRIBUTE_PARTICLE_PARAMS)),
            axis: f3(mesh.remove_attribute(ATTRIBUTE_PARTICLE_AXIS)),
            extra: f4(mesh.remove_attribute(ATTRIBUTE_PARTICLE_EXTRA)),
            local: f3(mesh.remove_attribute(ATTRIBUTE_PARTICLE_LOCAL)),
            normal: f3(mesh.remove_attribute(Mesh::ATTRIBUTE_NORMAL)),
            velocity: f3(mesh.remove_attribute(ATTRIBUTE_PARTICLE_VELOCITY)),
            indices,
        }
    }

    /// One copy of `template` for a particle: every vertex carries the
    /// particle, as a quad's corners do, plus its own place in the mesh. The
    /// UV rides in the corner as `(u - 0.5, 0.5 - v)`, which `sheet_uv` turns
    /// back into `(u, v)` for a one-frame sheet and into the frame's cell for
    /// a flipbook.
    fn push_mesh_copy(&mut self, template: &MeshTemplate, d: &ParticleDraw, color: Vec4) {
        let base = self.position.len() as u32;
        for (k, p) in template.positions.iter().enumerate() {
            let uv = template.uvs[k];
            self.position.push(d.center.to_array());
            self.corner.push([uv.x - 0.5, 0.5 - uv.y]);
            self.color.push(color.to_array());
            self.params.push(d.params);
            self.axis.push(d.axis.to_array());
            self.extra.push(d.extra);
            self.local.push(p.to_array());
            self.normal.push(template.normals[k].to_array());
            self.velocity.push(d.velocity.to_array());
        }
        self.indices.extend(template.indices.iter().map(|i| base + i));
    }

    /// A degenerate triangle with the mesh-particle columns, for a mesh
    /// renderer with nothing to draw.
    fn push_degenerate_mesh(&mut self) {
        let base = self.position.len() as u32;
        for _ in 0..3 {
            self.position.push([0.0; 3]);
            self.corner.push([0.0; 2]);
            self.color.push([0.0; 4]);
            self.params.push([0.0; 4]);
            self.axis.push([0.0; 3]);
            self.extra.push([0.0; 4]);
            self.local.push([0.0; 3]);
            self.normal.push([0.0, 1.0, 0.0]);
        }
        self.indices.extend_from_slice(&[base, base + 1, base + 2]);
    }

    fn push_quad(&mut self, center: Vec3, color: Vec4, params: [f32; 4], axis: Vec3, extra: [f32; 4], velocity: Vec3) {
        let base = self.position.len() as u32;
        for corner in CORNERS {
            self.position.push(center.to_array());
            self.corner.push(corner);
            self.color.push(color.to_array());
            self.params.push(params);
            self.axis.push(axis.to_array());
            self.extra.push(extra);
            self.velocity.push(velocity.to_array());
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn push_degenerate(&mut self) {
        self.push_quad(Vec3::ZERO, Vec4::ZERO, [0.0; 4], Vec3::ZERO, [0.0; 4], Vec3::ZERO);
    }

    fn store(mut self, mesh: &mut Mesh) {
        self.velocity.resize(self.position.len(), [0.0; 3]);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.position);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.corner);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.color);
        mesh.insert_attribute(ATTRIBUTE_PARTICLE_PARAMS, self.params);
        mesh.insert_attribute(ATTRIBUTE_PARTICLE_AXIS, self.axis);
        mesh.insert_attribute(ATTRIBUTE_PARTICLE_EXTRA, self.extra);
        mesh.insert_attribute(ATTRIBUTE_PARTICLE_VELOCITY, self.velocity);
        if !self.local.is_empty() {
            mesh.insert_attribute(ATTRIBUTE_PARTICLE_LOCAL, self.local);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normal);
        }
        mesh.insert_indices(Indices::U32(self.indices));
    }
}

/// Which sprite-sheet frame a particle shows.
///
/// Whole frames unless the emitter blends them, in which case the fraction is
/// kept and the shader crossfades frame `floor(f)` into the next by it. Over a
/// lifetime a blended sheet runs `0..frames - 1` rather than `0..frames`, so
/// the last frame is reached exactly at death instead of being blended toward
/// a frame past the end.
pub fn frame(render: &RenderSettings, age: f32, lifetime: f32) -> f32 {
    let frames = render.sprite_sheet.frame_count();
    if frames <= 1 {
        return 0.0;
    }
    let n = frames as f32;
    let life = if lifetime > 0.0 { (age / lifetime).clamp(0.0, 1.0) } else { 0.0 };
    match (render.sprite_sheet.mode, render.frame_blend) {
        (SheetMode::OverLifetime, false) => ((life * n) as u32).min(frames - 1) as f32,
        (SheetMode::OverLifetime, true) => life * (n - 1.0),
        (SheetMode::Fps(fps), false) => {
            let k = (age * fps.max(0.0)) as u64;
            (k % frames as u64) as u32 as f32
        }
        (SheetMode::Fps(fps), true) => {
            let f = (age * fps.max(0.0)).rem_euclid(n);
            if f.is_finite() {
                f
            } else {
                0.0
            }
        }
    }
}

/// A stable 0..1 random per particle for the shader's noise, from its spawn
/// index. Not the graph's randoms (those are keyed by the emitter seed and a
/// salt), only something that keeps two particles' flames from matching.
fn particle_seed(id: u32) -> f32 {
    crate::random::unit_float(crate::random::pcg(id ^ 0x9E37_79B9))
}

/// Write one emitter's particles into its mesh. `to_mesh` takes a particle's
/// simulation-space position into mesh space. Returns the AABB, or `None` when
/// there is nothing to draw.
pub fn write_mesh(
    mesh: &mut Mesh,
    particles: &ParticleBuffer,
    render: &RenderSettings,
    to_mesh: Affine3A,
) -> Option<Aabb> {
    write_mesh_sorted(mesh, particles, render, to_mesh, None)
}

/// [`write_mesh`], with the quads written back to front from `eye` (in mesh
/// space) when there is one. A GPU draws a mesh's triangles and blends them in
/// index order, so the order written here is the order composited; ties keep
/// buffer order, so two particles at one distance do not swap from frame to
/// frame. Ribbons ignore it: a strip is drawn in trail order.
pub fn write_mesh_sorted(
    mesh: &mut Mesh,
    particles: &ParticleBuffer,
    render: &RenderSettings,
    to_mesh: Affine3A,
    eye: Option<Vec3>,
) -> Option<Aabb> {
    write_particles(mesh, particles, render, to_mesh, eye, None)
}

/// [`write_mesh_sorted`] for mesh particles: one copy of `template` per
/// particle instead of a quad, for the vertex shader to turn and size. With
/// no template (a model still loading) it writes nothing and hides.
pub fn write_mesh_particles(
    mesh: &mut Mesh,
    particles: &ParticleBuffer,
    render: &RenderSettings,
    to_mesh: Affine3A,
    eye: Option<Vec3>,
    template: &MeshTemplate,
) -> Option<Aabb> {
    write_particles(mesh, particles, render, to_mesh, eye, Some(template))
}

fn write_particles(
    mesh: &mut Mesh,
    particles: &ParticleBuffer,
    render: &RenderSettings,
    to_mesh: Affine3A,
    eye: Option<Vec3>,
    template: Option<&MeshTemplate>,
) -> Option<Aabb> {
    let mut scratch = MeshScratch::take(mesh);
    // A mesh turns by `orient`, which reads the velocity or the axis through
    // the facing slot exactly as a quad does (see `effective_facing`).
    let facing = match template {
        Some(_) => render.effective_facing(Dimension::ThreeD),
        None => render.facing,
    };
    if template.is_none() && render.facing == Facing::Ribbon {
        let bounds = ribbon::write(&mut scratch, particles, render, to_mesh);
        let aabb = if bounds.any {
            Some(Aabb::from_min_max(bounds.min, bounds.max))
        } else {
            scratch.push_degenerate();
            None
        };
        scratch.store(mesh);
        return aabb;
    }
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for_each_draw(particles, render, facing, to_mesh, eye, |d| {
        let color = particles.color[d.index];
        let r = match template {
            Some(t) => {
                scratch.push_mesh_copy(t, &d, color);
                // The copy is scaled by (x, y, x) and turned any way, so its
                // largest scale times the mesh's own radius covers it.
                d.size.abs().max_element() * t.radius
            }
            None => {
                scratch.push_quad(d.center, color, d.params, d.axis, d.extra, d.velocity);
                // Half the diagonal covers any rotation of the quad.
                0.5 * d.size.abs().length()
            }
        };
        min = min.min(d.center - Vec3::splat(r));
        max = max.max(d.center + Vec3::splat(r));
    });
    let aabb = if scratch.position.is_empty() {
        if template.is_some() {
            scratch.push_degenerate_mesh();
        } else {
            scratch.push_degenerate();
        }
        None
    } else {
        Some(Aabb::from_min_max(min, max))
    };
    scratch.store(mesh);
    aabb
}

/// Write a mesh-particle renderer: one copy of its template per particle,
/// whichever material draws it. A template still loading draws nothing and
/// hides.
pub fn write_mesh_renderer(
    mesh: &mut Mesh,
    particles: &ParticleBuffer,
    render: &RenderSettings,
    to_mesh: Affine3A,
    eye: Option<Vec3>,
    mesh_particles: &MeshParticles,
) -> Option<Aabb> {
    let empty = ParticleBuffer::default();
    let (particles, template) = match mesh_particles.template.as_deref() {
        Some(t) => (particles, t),
        None => (&empty, &MeshTemplate::default()),
    };
    write_mesh_particles(mesh, particles, render, to_mesh, eye, template)
}

/// One particle as the renderer writes it, in mesh space.
pub(crate) struct ParticleDraw {
    pub index: usize,
    pub center: Vec3,
    /// The velocity (velocity-aligned) or the fixed axis, else zero.
    pub axis: Vec3,
    /// The size after a velocity stretch.
    pub size: Vec2,
    /// `(size.x, size.y, rotation, frame)`.
    pub params: [f32; 4],
    /// `(seed, life, 0, 0)`.
    pub extra: [f32; 4],
    /// The particle's velocity in mesh space.
    pub velocity: Vec3,
}

/// Every drawable particle, back to front from `eye` when there is one, else
/// in the emitter's age order ([`crate::asset::DrawOrder`]) or buffer order,
/// with what the vertex needs worked out for `facing`. A particle at a non-finite
/// place is skipped rather than drawn at NaN.
pub(crate) fn for_each_draw(
    particles: &ParticleBuffer,
    render: &RenderSettings,
    facing: Facing,
    to_mesh: Affine3A,
    eye: Option<Vec3>,
    mut f: impl FnMut(ParticleDraw),
) {
    let fixed_axis = match facing {
        Facing::FixedAxis(a) => Vec3::from_array(a),
        _ => Vec3::ZERO,
    };
    // The age orders need no camera, so they are read from the settings
    // here; `eye` arrives only for a depth sort. Ties keep buffer order in
    // both, so particles born in one step do not swap from frame to frame.
    let by_age = |youngest_first: bool| -> Vec<usize> {
        let mut keyed: Vec<(f32, usize)> = (0..particles.len()).map(|i| (particles.age[i], i)).collect();
        if youngest_first {
            keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
        } else {
            keyed.sort_by(|a, b| b.0.total_cmp(&a.0));
        }
        keyed.into_iter().map(|(_, i)| i).collect()
    };
    let order: Vec<usize> = match (eye, render.draw_order(crate::asset::Dimension::ThreeD)) {
        (Some(eye), _) => {
            let mut keyed: Vec<(f32, usize)> = (0..particles.len())
                .map(|i| (to_mesh.transform_point3(particles.position[i]).distance_squared(eye), i))
                .collect();
            keyed.sort_by(|a, b| b.0.total_cmp(&a.0));
            keyed.into_iter().map(|(_, i)| i).collect()
        }
        (None, crate::asset::DrawOrder::OldestFirst) => by_age(false),
        (None, crate::asset::DrawOrder::YoungestFirst) => by_age(true),
        (None, _) => (0..particles.len()).collect(),
    };
    for i in order {
        let center = to_mesh.transform_point3(particles.position[i]);
        let velocity = to_mesh.transform_vector3(particles.velocity[i]);
        let axis = match facing {
            Facing::CameraBillboard | Facing::Ribbon => Vec3::ZERO,
            Facing::VelocityAligned => velocity,
            Facing::FixedAxis(_) => fixed_axis,
        };
        if !center.is_finite() || !axis.is_finite() {
            continue;
        }
        let velocity = if velocity.is_finite() { velocity } else { Vec3::ZERO };
        let mut size = particles.size[i];
        if facing == Facing::VelocityAligned && render.stretch > 0.0 && render.stretch.is_finite() {
            // In mesh space, like the axis the shader stretches along, so an
            // entity's scale lengthens the streak as it does the quad.
            size.y += axis.length() * render.stretch;
        }
        let params = [size.x, size.y, particles.rotation[i], frame(render, particles.age[i], particles.lifetime[i])];
        let life = if particles.lifetime[i] > 0.0 {
            (particles.age[i] / particles.lifetime[i]).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let extra = [particle_seed(particles.id[i]), life, 0.0, 0.0];
        f(ParticleDraw { index: i, center, axis, size, params, extra, velocity });
    }
}

/// The world position of the camera an effect on `layers` sorts against: the
/// active 3D camera that sees those layers, the highest `order` first (the
/// editor's preview renders after the scene viewport and sees only its own
/// layer, so a preview effect sorts against the preview's camera). `None`
/// when no camera sees it.
pub fn sort_eye<'a>(
    cameras: impl Iterator<Item = (&'a Camera, &'a GlobalTransform, Option<&'a RenderLayers>)>,
    layers: Option<&RenderLayers>,
) -> Option<Vec3> {
    let default_layers = RenderLayers::default();
    let layers = layers.unwrap_or(&default_layers);
    cameras
        .filter(|(camera, _, cam_layers)| camera.is_active && cam_layers.unwrap_or(&default_layers).intersects(layers))
        .max_by_key(|(camera, _, _)| camera.order)
        .map(|(_, global, _)| global.translation())
}

/// The transform from an emitter's simulation space into its renderer's mesh
/// space. `None` when the entity's transform cannot be inverted (a zero scale),
/// in which case there is nothing sensible to draw.
pub fn sim_to_mesh(
    space: SimulationSpace,
    entity_global: &GlobalTransform,
    renderer_local: &Transform,
) -> Option<Affine3A> {
    let local = renderer_local.compute_affine();
    let m = match space {
        SimulationSpace::Local => local.inverse(),
        SimulationSpace::World => (entity_global.affine() * local).inverse(),
    };
    m.is_finite().then_some(m)
}
