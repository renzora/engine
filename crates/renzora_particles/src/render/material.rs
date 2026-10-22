//! The two particle materials: a `Material` for 3D and a `Material2d` for 2D.
//!
//! Both draw the per-emitter mesh `super::write_meshes` builds, whose vertices
//! carry a particle's centre rather than a corner position. The WESL vertex
//! shader expands each quad itself (camera-facing, velocity-aligned or around a
//! fixed axis), so the CPU never needs the camera and one mesh is correct in
//! every viewport and preview at once.
//!
//! These are the built-in look: the soft dot, sprites and flipbooks, the
//! procedural flame and smoke, erosion and soft particles. A particle wearing a
//! project `.material` draws through [`super::surface`] instead, an
//! `ExtendedMaterial<StandardMaterial, _>` that keeps the material's own
//! fragment shader and borrows this module's vertex expansion. The built-in
//! look stays a plain `Material` on purpose rather than becoming a default
//! `.material` on that pipeline:
//!
//! - its bind group is a uniform, one texture and a sampler, where the
//!   extended one carries all of `StandardMaterial` (around a dozen textures)
//!   plus the graph's nine slots, per emitter, to draw a dot;
//! - its fragment shader is shared with 2D (the SHARED block), which has no
//!   `StandardMaterial` to extend;
//! - additive particles write premultiplied colour with alpha 0 and soft
//!   particles sample the depth prepass, neither of which the PBR fragment does.
//!
//! ## Blending
//!
//! 3D gets every mode from Bevy's own pipeline key: `AlphaMode::Blend`, `Add`,
//! `Premultiplied`, `Multiply` and `AlphaToCoverage`. Bevy draws `Add` with
//! premultiplied blending and expects the shader to output alpha 0, and
//! `Multiply` with `dst * src + dst * (1 - src.a)`, which wants premultiplied
//! colour; the fragment shader branches on the blend code in the uniform to
//! produce what each pipeline expects. 2D only has `AlphaMode2d::Blend`, so
//! additive, premultiplied and multiply need the blend state changed in
//! `specialize`, keyed on [`Particle2dKey`] through `bind_group_data` so the
//! variants get pipelines of their own.
//!
//! `Opaque`, `AlphaMask` and `AlphaToCoverage` write depth: the particles draw
//! in the opaque and alpha-mask phases and are ordered against each other and
//! the scene by the depth buffer, so they never need a sort. The shader
//! discards below the cutoff itself and writes alpha 1, except under coverage,
//! where it writes the alpha sharpened around the cutoff for the MSAA resolve
//! to turn into a smooth edge.
//!
//! ## Deferred
//!
//! The depth-written blends (`Opaque`, `AlphaMask`, `AlphaToCoverage`) draw
//! through the G-buffer on a deferred camera, so a solid particle is lit by
//! the deferred lighting pass and seen by SSR, SSAO and Solari like any other
//! surface. Bevy's deferred G-buffer pass is its prepass pipeline with
//! `DEFERRED_PREPASS` set, run with the material's `deferred_vertex_shader`
//! and `deferred_fragment_shader`; both are this file, so the vertex stage is
//! the same `PREPASS_PIPELINE` expansion the depth prepass and shadows use
//! (quads, meshes, ribbons and tubes alike), and the fragment, under
//! `DEFERRED_PREPASS`, packs the particle into the G-buffer with Bevy's own
//! `deferred_gbuffer_from_pbr_input`:
//!
//! - lit: the colour times the texture as the base colour, the emitter's
//!   roughness and metallic, the card's or the mesh's normal (flipped on a
//!   mesh's back face, as the forward lighting does), and the HDR overflow of
//!   the colour (the Emission pin past 1) as emissive, because the G-buffer's
//!   base colour is 8 bits per channel and would clip it. A procedural flame
//!   is light, so its whole colour goes to emissive and its base is black;
//!   smoke is a surface like a sprite.
//! - unlit: Bevy's unlit flag, which the deferred lighting pass draws as the
//!   colour unshaded (it carries it in the emissive channel, at full HDR).
//!
//! `AlphaToCoverage` goes deferred too, because Bevy routes it through the
//! opaque phase; a deferred camera runs without MSAA (the G-buffer is single
//! sampled), so there it is a mask at the cutoff, which is what it already
//! was on a camera without MSAA.
//!
//! The transparent blends always draw forward, after the deferred lighting,
//! as in every deferred engine: a G-buffer holds one surface per pixel and a
//! blend needs what is behind it. Bevy routes a transparent alpha mode to the
//! forward transparent pass whatever the render method says, and the material
//! asks for `Forward` there anyway ([`RenderSettings::effective_render_method`]).
//!
//! Which route a solid particle takes is [`RenderRoute`]: `Auto` (Bevy's
//! `OpaqueRendererMethod::Auto`, the project's default, which is what every
//! `StandardMaterial` follows, so a forward project draws exactly as before),
//! `Forward`, or `Deferred`, which is honoured only while every 3D camera has
//! a deferred prepass ([`resolve_render_routes`]).
//!
//! ## The prepass and shadows
//!
//! Both are on, with the shader's own entry points under `PREPASS_PIPELINE`:
//! the prepass vertex shader expands the quad or the mesh exactly as the main
//! one does (the same functions, called with the view the pass renders), and
//! the prepass fragment runs the particle's own colour to discard what the
//! main pass would. Bevy keeps blended materials out of the depth prepass by
//! itself, so what lands there is the depth-written blends: their depth,
//! normals and motion vectors, which SSAO, TAA, motion blur and other
//! emitters' soft fades read.
//!
//! Shadows are per renderer, by `NotShadowCaster` ([`RenderSettings::casts_shadows`]):
//! a shadow view expands each quad toward the light, the way a camera-facing
//! card is lit from the side the light sees.
//!
//! **Motion vectors** carry the particle's own travel: the previous position
//! is the current one minus the velocity times the frame's delta, scaled by
//! the playback rate in `params.motion.x` (0 when paused, so a frozen effect
//! does not smear under motion blur). A `Local` emitter's previous position
//! goes through the entity's previous transform too, so a torch carried past
//! the camera blurs with the hand; a `World` one does not, because its
//! particles do not move with the entity. Spin, growth and a billboard's turn
//! toward a moving camera are left out: the vertex carries no previous
//! rotation or size, and storing them would cost the GPU pool a third of its
//! bandwidth for an effect TAA mostly absorbs anyway.
//!
//! **OIT** stays off. Bevy's OIT resolves its layers with straight "over"
//! blending, which would composite additive and premultiplied particles
//! wrongly; the forward transparent pass with an emitter sort (and the
//! order-independent additive and multiply modes) gives the right picture on a
//! camera with OIT as without, where enabling it with a shader that never
//! writes the OIT buffers would drop the particles entirely.
//!
//! ## Mesh particles
//!
//! A [`Particle3dKey`] with `mesh` set gives the pipeline two more vertex
//! attributes (each vertex's position and normal in the source mesh,
//! locations 6 and 7) and the `MESH_PARTICLES` def, under which the vertex
//! shader places, turns and sizes a mesh copy instead of expanding a quad.
//! The mesh is still written by the CPU, one copy of the source mesh per
//! particle, so everything else (sorting, bounds, the uniform) is the quad
//! path's.
//!
//! ## Lit particles
//!
//! `surface.x` turns on Bevy's own `apply_pbr_lighting` in the fragment
//! shader, with the view's lights, shadows, ambient and environment map. A
//! uniform rather than a def for the same reason as the shading modes: one
//! pipeline per blend mode, and the branch is uniform across the draw.
//! Emitters sort against each other and the scene as whole meshes; particles
//! within one alpha-blended emitter draw in buffer order unless the emitter
//! sorts (`RenderSettings::sort`), when they are written back to front.
//!
//! ## Soft particles
//!
//! A soft particle fades where it nears the opaque depth behind it, which
//! needs that depth readable in the transparent pass. It is, safely, whenever
//! the view has a `DepthPrepass`: Bevy then copies the depth buffer into a
//! separate prepass texture (after the prepass in forward, after the G-buffer
//! pass in deferred) and binds it in the main pass's view bind group under the
//! `DEPTH_PREPASS` shader def, so the shader samples a texture the pass is not
//! also writing. The 3D shader reads it only under that def, and the def and
//! the binding come from the same view key, so a camera without a depth
//! prepass compiles the fade out and draws hard edges rather than failing.
//! WebGL2 is excluded too (`!WEBGL2`): Bevy cannot copy a depth texture there,
//! so the prepass texture would hold nothing. Every engine camera carries
//! `DepthPrepass`. 2D has no prepass at all, so the 2D shader has no soft path.

use bevy::mesh::{MeshVertexAttribute, VertexFormat};
use bevy::prelude::*;
use bevy::render::render_resource::ShaderType;
#[cfg(any(feature = "render_3d", feature = "render_2d"))]
use bevy::{
    mesh::MeshVertexBufferLayoutRef,
    render::render_resource::{AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError},
    shader::ShaderRef,
};

use crate::asset::{BlendMode, Dimension, Facing, RenderMethod, RenderSettings, Shading, SheetMode, SimulationSpace};

/// `vec4(size.x, size.y, rotation, frame)`.
pub const ATTRIBUTE_PARTICLE_PARAMS: MeshVertexAttribute =
    MeshVertexAttribute::new("Vertex_ParticleParams", 0x5246_5801, VertexFormat::Float32x4);
/// The direction a velocity-aligned or fixed-axis quad stretches along, in mesh
/// space. Zero for a camera-facing quad.
pub const ATTRIBUTE_PARTICLE_AXIS: MeshVertexAttribute =
    MeshVertexAttribute::new("Vertex_ParticleAxis", 0x5246_5802, VertexFormat::Float32x3);
/// `vec4(seed, life, 0, 0)`: a stable 0..1 random per particle and its age
/// over its lifetime. The procedural shading needs both: the seed so no two
/// flames share a noise pattern, the life so erosion eats a particle as it
/// ages. Sent per vertex rather than derived in the shader because a vertex
/// shader has no particle id to hash.
pub const ATTRIBUTE_PARTICLE_EXTRA: MeshVertexAttribute =
    MeshVertexAttribute::new("Vertex_ParticleExtra", 0x5246_5803, VertexFormat::Float32x4);
/// The particle's velocity in mesh space, 3D only. Separate from the axis,
/// which carries it only for a velocity-aligned facing: motion vectors and a
/// material graph's Particle Velocity node want it whatever the facing. A
/// ribbon vertex carries zero (the column is padded), since a strip point is
/// a place on a trail rather than a moving particle.
pub const ATTRIBUTE_PARTICLE_VELOCITY: MeshVertexAttribute =
    MeshVertexAttribute::new("Vertex_ParticleVelocity", 0x5246_5805, VertexFormat::Float32x3);

pub const SHADER_3D: &str = "embedded://renzora_particles/shaders/particle3d.wesl";
pub const SHADER_2D: &str = "embedded://renzora_particles/shaders/particle2d.wesl";

/// The uniform both shaders read. `Reflect` because the particle-surface
/// extension that carries it is reflected, as Bevy's extended materials are.
#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType, Reflect)]
pub struct ParticleMaterialParams {
    /// `x` columns, `y` rows of the sprite sheet, `z` 1 for a fixed-rate
    /// sheet, `w` its frames per second.
    pub sheet: Vec4,
    /// `x` blend ([`blend_code`]), `y` facing (0 camera, 1 velocity, 2 fixed
    /// axis, 3 ribbon, 4 tube ribbon; for a mesh, what its orientation reads, see
    /// `RenderSettings::effective_facing`), `z` 1 when a texture is bound, 0
    /// for the soft dot, 2 for solid white (an untextured mesh), `w` the
    /// velocity stretch.
    pub mode: Vec4,
    /// `x` shading (0 sprite, 1 procedural flame, 2 procedural smoke), `y`
    /// erosion, `z` soft-particle distance (0 off), `w` 1 to blend sprite-sheet
    /// frames.
    ///
    /// A uniform rather than shader defs: the branch is the same for every
    /// fragment of a draw, so it costs next to nothing, and one shader keeps
    /// one pipeline per blend mode instead of multiplying them by three.
    pub shading: Vec4,
    /// `FixedAxis` only: the axis, in the emitter's space.
    ///
    /// This, `gpu`, `sheet.w` and `mode.w` are read only by the GPU backend's
    /// vertex fetch, which works out per vertex what the CPU path writes into
    /// the mesh (the axis, the stretched size, the sheet frame). The CPU
    /// path's shader declares them and ignores them.
    pub axis: Vec4,
    /// GPU backend only: `x` 1 to draw in sorted order, `y` the absolute word
    /// index of the sorted list in the state buffer.
    pub gpu: Vec4,
    /// `x` 1 to light with the scene (3D), `y` roughness, `z` metallic, `w`
    /// the alpha-mask cutoff.
    pub surface: Vec4,
    /// `x` 1 for mesh particles (3D), `y` 1 when sizes are in screen pixels.
    pub shape: Vec4,
    /// Motion vectors (3D prepass): `x` the playback rate, simulated seconds
    /// per second of frame (the time scale while playing, 0 while paused or
    /// stopped), `y` 1 for a `World` emitter, whose previous position does
    /// not follow the entity's previous transform. Written when the playback
    /// changes, not every frame, so a playing effect costs no asset churn.
    pub motion: Vec4,
}

impl ParticleMaterialParams {
    /// The uniform for a 3D emitter. See [`Self::for_dimension`].
    pub fn new(render: &RenderSettings, has_texture: bool) -> Self {
        Self::for_dimension(render, has_texture, Dimension::ThreeD)
    }

    /// The uniform for an emitter of `dimension`, which decides whether the
    /// mesh and lighting settings apply (they are 3D only).
    pub fn for_dimension(render: &RenderSettings, has_texture: bool, dimension: Dimension) -> Self {
        let shading = match render.shading {
            Shading::Sprite => 0.0,
            Shading::ProceduralFlame => 1.0,
            Shading::ProceduralSmoke => 2.0,
        };
        let blend = blend_code(render.blend);
        let mesh = render.draws_mesh(dimension);
        let facing_setting = render.effective_facing(dimension);
        let facing = match facing_setting {
            Facing::CameraBillboard => 0.0,
            Facing::VelocityAligned => 1.0,
            Facing::FixedAxis(_) => 2.0,
            // 4 is a tube: the ribbon's points, pushed round the strip
            // rather than across it (3D only; see `RenderSettings::draws_tube`).
            Facing::Ribbon if render.draws_tube(dimension) => 4.0,
            Facing::Ribbon => 3.0,
        };
        let (sheet_mode, fps) = match render.sprite_sheet.mode {
            SheetMode::OverLifetime => (0.0, 0.0),
            SheetMode::Fps(fps) => (1.0, finite_or_zero(fps).max(0.0)),
        };
        let stretch = if render.stretch.is_finite() && render.stretch > 0.0 { render.stretch } else { 0.0 };
        let axis = match facing_setting {
            Facing::FixedAxis(a) if a.iter().all(|c| c.is_finite()) => Vec4::new(a[0], a[1], a[2], 0.0),
            _ => Vec4::ZERO,
        };
        // A mesh without a texture is solid (2) rather than the soft dot (0),
        // which would be a blotch smeared over the mesh's UVs; so is a tube,
        // whose UVs run round it and would fade one side of it to nothing.
        let texture = match (has_texture, mesh || render.draws_tube(dimension)) {
            (true, _) => 1.0,
            (false, true) => 2.0,
            (false, false) => 0.0,
        };
        // Additive colour is emitted light, which lighting would only dim; the
        // shader has no straight colour to light there either. A multiply
        // layer is a filter over what is behind, not a surface of its own.
        let lit = render.lighting.lit
            && dimension == Dimension::ThreeD
            && !matches!(render.blend, BlendMode::Additive | BlendMode::Multiply);
        Self {
            sheet: Vec4::new(
                render.sprite_sheet.columns.max(1) as f32,
                render.sprite_sheet.rows.max(1) as f32,
                sheet_mode,
                fps,
            ),
            mode: Vec4::new(blend, facing, texture, stretch),
            surface: Vec4::new(
                if lit { 1.0 } else { 0.0 },
                finite_or_zero(render.lighting.roughness).clamp(0.045, 1.0),
                finite_or_zero(render.lighting.metallic).clamp(0.0, 1.0),
                finite_or_zero(render.alpha_cutoff).clamp(0.0, 1.0),
            ),
            shape: Vec4::new(
                if mesh { 1.0 } else { 0.0 },
                if render.screen_space_size { 1.0 } else { 0.0 },
                0.0,
                0.0,
            ),
            axis,
            gpu: Vec4::ZERO,
            shading: Vec4::new(
                shading,
                finite_or_zero(render.erosion).clamp(0.0, 1.0),
                finite_or_zero(render.soft_distance).max(0.0),
                if render.frame_blend && render.sprite_sheet.frame_count() > 1 { 1.0 } else { 0.0 },
            ),
            motion: Vec4::new(1.0, 0.0, 0.0, 0.0),
        }
    }

    /// The same uniform for an emitter simulating in `space`.
    pub fn in_space(mut self, space: SimulationSpace) -> Self {
        self.motion.y = if space == SimulationSpace::World { 1.0 } else { 0.0 };
        self
    }
}

fn finite_or_zero(v: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

/// `params.mode.x`: 0 alpha, 1 additive, 2 premultiplied, 3 opaque, 4 alpha
/// mask, 5 multiply, 6 alpha to coverage. The shaders read it as an integer
/// (`blend_mode()`), so the order carries no meaning beyond being stable.
pub fn blend_code(blend: BlendMode) -> f32 {
    match blend {
        BlendMode::Alpha => 0.0,
        BlendMode::Additive => 1.0,
        BlendMode::Premultiplied => 2.0,
        BlendMode::Opaque => 3.0,
        BlendMode::AlphaMask => 4.0,
        BlendMode::Multiply => 5.0,
        BlendMode::AlphaToCoverage => 6.0,
    }
}

/// The 3D pipeline's alpha mode for a blend. `Opaque`, `Mask` and
/// `AlphaToCoverage` put the particles in Bevy's opaque and alpha-mask
/// phases, which write depth and are ordered by the depth buffer instead of a
/// sort.
#[cfg(feature = "render_3d")]
pub fn alpha_mode_3d(blend: BlendMode, cutoff: f32) -> AlphaMode {
    match blend {
        BlendMode::Alpha => AlphaMode::Blend,
        BlendMode::Additive => AlphaMode::Add,
        BlendMode::Premultiplied => AlphaMode::Premultiplied,
        BlendMode::Opaque => AlphaMode::Opaque,
        BlendMode::AlphaMask => AlphaMode::Mask(finite_or_zero(cutoff).clamp(0.0, 1.0)),
        BlendMode::Multiply => AlphaMode::Multiply,
        BlendMode::AlphaToCoverage => AlphaMode::AlphaToCoverage,
    }
}

/// The render method a 3D particle material asks for and the one it gets.
///
/// `requested` is the emitter's [`RenderSettings::effective_render_method`];
/// `method` is what the material hands Bevy. They differ only for a
/// `Deferred` request while some 3D camera has no deferred prepass: Bevy
/// leaves a deferred material out of the forward main pass and, on a view
/// with a prepass but no deferred phase, panics queueing it, so the request
/// waits as `Forward` until every camera can draw it ([`resolve_render_routes`]).
/// `Auto` needs no such guard: it follows the project's default method, and
/// the engine gives every 3D camera a deferred prepass exactly when that
/// default is deferred, which is the guarantee `StandardMaterial` relies on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RenderRoute {
    pub requested: RenderMethod,
    pub method: RenderMethod,
}

impl RenderRoute {
    /// A fresh route for `requested`. A `Deferred` request starts forward and
    /// is promoted by [`resolve_render_routes`] the same frame if it can be,
    /// so a material is never deferred before anyone has checked the cameras.
    pub fn new(requested: RenderMethod) -> Self {
        let method = if requested == RenderMethod::Deferred { RenderMethod::Forward } else { requested };
        Self { requested, method }
    }

    /// The route for an emitter's settings.
    pub fn of(render: &RenderSettings) -> Self {
        Self::new(render.effective_render_method())
    }

    /// This route once the cameras are known: a `Deferred` request is
    /// deferred while `deferred_cameras` holds, forward otherwise.
    pub fn resolved(self, deferred_cameras: bool) -> Self {
        let method = match self.requested {
            RenderMethod::Deferred if !deferred_cameras => RenderMethod::Forward,
            other => other,
        };
        Self { method, ..self }
    }

    /// Bevy's method for the material.
    #[cfg(feature = "render_3d")]
    pub fn opaque(self) -> bevy::material::OpaqueRendererMethod {
        use bevy::material::OpaqueRendererMethod;
        match self.method {
            RenderMethod::Auto => OpaqueRendererMethod::Auto,
            RenderMethod::Forward => OpaqueRendererMethod::Forward,
            RenderMethod::Deferred => OpaqueRendererMethod::Deferred,
        }
    }
}

/// A 3D particle material whose [`RenderRoute`] the camera check can reach.
pub trait ParticleRouteMut {
    fn route(&self) -> RenderRoute;
    fn set_route(&mut self, route: RenderRoute);
}

/// Whether every active 3D camera has a deferred prepass, so a deferred
/// material can be drawn by all of them.
#[cfg(feature = "render_3d")]
pub fn cameras_draw_deferred<'a>(cameras: impl IntoIterator<Item = (&'a Camera, bool)>) -> bool {
    cameras.into_iter().filter(|(c, _)| c.is_active).all(|(_, deferred)| deferred)
}

/// Promote or demote every `Deferred` request of material `M` to what the
/// cameras allow (see [`RenderRoute`]). Only a material whose route changes
/// is touched, so the steady state writes nothing and respecializes nothing.
///
/// Runs after the particle systems (which create the materials) in
/// `PostUpdate`, so a material added this frame is resolved before the render
/// world extracts it. A camera spawned later in the same frame without a
/// deferred prepass would see one frame of a deferred material; nothing in
/// the engine spawns cameras there.
#[cfg(feature = "render_3d")]
pub fn resolve_render_routes<M: Asset + ParticleRouteMut>(
    cameras: Query<(&Camera, Has<bevy::core_pipeline::prepass::DeferredPrepass>), With<Camera3d>>,
    materials: Option<ResMut<Assets<M>>>,
) {
    let Some(mut materials) = materials else { return };
    let deferred = cameras_draw_deferred(cameras.iter());
    let stale: Vec<(AssetId<M>, RenderRoute)> = materials
        .iter()
        .filter_map(|(id, m)| {
            let route = m.route();
            let resolved = route.resolved(deferred);
            (resolved != route).then_some((id, resolved))
        })
        .collect();
    for (id, route) in stale {
        if let Some(mut m) = materials.get_mut(id) {
            m.set_route(route);
        }
    }
}

/// `vec4(local position, 0)`: a mesh particle's vertex in the mesh's own
/// space, before the particle's orientation and size. CPU path only; the GPU
/// path reads the same from its pool mesh.
pub const ATTRIBUTE_PARTICLE_LOCAL: MeshVertexAttribute =
    MeshVertexAttribute::new("Vertex_ParticleLocal", 0x5246_5804, VertexFormat::Float32x3);

/// Which vertex layout a particle pipeline reads: quads, or mesh particles
/// (which add each vertex's position and normal in the source mesh). A
/// material key rather than a uniform, because it changes the vertex buffer,
/// which a uniform cannot.
///
/// `method` is the route the material takes ([`RenderRoute::method`]). Bevy
/// already keys the G-buffer pipeline apart through `DEFERRED_PREPASS`; it is
/// in the key too so a forward and a deferred emitter of the same type never
/// share a specialized pipeline or a batch, and a route switch (a camera
/// gaining its deferred prepass) respecializes the material at once rather
/// than waiting on something else to change.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Particle3dKey {
    pub mesh: bool,
    pub method: RenderMethod,
}

/// The shader def that switches the 3D particle shader to mesh particles.
pub const MESH_PARTICLES_DEF: &str = "MESH_PARTICLES";

/// The vertex buffer the CPU path's pipelines read. Locations must match the
/// shaders'. `velocity` adds location 8, which only the 3D shaders declare.
#[cfg(any(feature = "render_3d", feature = "render_2d"))]
pub(crate) fn vertex_layout(
    layout: &MeshVertexBufferLayoutRef,
    mesh: bool,
    velocity: bool,
) -> Result<bevy::mesh::VertexBufferLayout, SpecializedMeshPipelineError> {
    let mut attributes = vec![
        Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
        Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
        Mesh::ATTRIBUTE_COLOR.at_shader_location(2),
        ATTRIBUTE_PARTICLE_PARAMS.at_shader_location(3),
        ATTRIBUTE_PARTICLE_AXIS.at_shader_location(4),
        ATTRIBUTE_PARTICLE_EXTRA.at_shader_location(5),
    ];
    if mesh {
        attributes.push(ATTRIBUTE_PARTICLE_LOCAL.at_shader_location(6));
        attributes.push(Mesh::ATTRIBUTE_NORMAL.at_shader_location(7));
    }
    if velocity {
        attributes.push(ATTRIBUTE_PARTICLE_VELOCITY.at_shader_location(8));
    }
    Ok(layout.0.get_layout(&attributes)?)
}

#[cfg(any(feature = "render_3d", feature = "render_2d"))]
fn apply_common(
    descriptor: &mut RenderPipelineDescriptor,
    layout: &MeshVertexBufferLayoutRef,
    mesh: bool,
    velocity: bool,
) -> Result<(), SpecializedMeshPipelineError> {
    descriptor.vertex.buffers = vec![vertex_layout(layout, mesh, velocity)?];
    if mesh {
        add_def(descriptor, MESH_PARTICLES_DEF);
    }
    // A rotated or velocity-aligned quad can face away from the camera in its
    // own winding; it is still the front as far as anyone looking is concerned.
    // Meshes are drawn from both sides too, because the one that most wants to
    // be a mesh particle, a leaf, is a single-sided card; the shader flips the
    // normal on a back face so it is lit as the side being seen.
    descriptor.primitive.cull_mode = None;
    Ok(())
}

/// Add a shader def to both stages.
#[cfg(any(feature = "render_3d", feature = "render_2d"))]
pub(crate) fn add_def(descriptor: &mut RenderPipelineDescriptor, def: &'static str) {
    descriptor.vertex.shader_defs.push(def.into());
    if let Some(fragment) = descriptor.fragment.as_mut() {
        fragment.shader_defs.push(def.into());
    }
}

/// Whether `descriptor` is a prepass or shadow pipeline (Bevy builds both from
/// the prepass pipeline, labelled `prepass_pipeline`).
#[cfg(feature = "material_graph")]
pub(crate) fn is_prepass(descriptor: &RenderPipelineDescriptor) -> bool {
    descriptor.label.as_deref().is_some_and(|l| l.contains("prepass") || l.contains("shadow"))
}

#[cfg(feature = "render_3d")]
pub use three::ParticleMaterial3d;

#[cfg(feature = "render_3d")]
mod three {
    use super::*;
    use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey};

    #[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
    #[bind_group_data(Particle3dKey)]
    pub struct ParticleMaterial3d {
        #[uniform(0)]
        pub params: ParticleMaterialParams,
        #[texture(1)]
        #[sampler(2)]
        pub texture: Option<Handle<Image>>,
        pub blend: BlendMode,
        /// [`BlendMode::AlphaMask`]'s cutoff, for the pipeline's alpha mode.
        pub cutoff: f32,
        /// Draws mesh particles (the mesh vertex layout).
        pub mesh: bool,
        /// Forward or the G-buffer; see the module docs.
        pub route: RenderRoute,
    }

    impl From<&ParticleMaterial3d> for Particle3dKey {
        fn from(material: &ParticleMaterial3d) -> Self {
            Self { mesh: material.mesh, method: material.route.method }
        }
    }

    impl ParticleRouteMut for ParticleMaterial3d {
        fn route(&self) -> RenderRoute {
            self.route
        }
        fn set_route(&mut self, route: RenderRoute) {
            self.route = route;
        }
    }

    impl Material for ParticleMaterial3d {
        fn vertex_shader() -> ShaderRef {
            SHADER_3D.into()
        }

        fn fragment_shader() -> ShaderRef {
            SHADER_3D.into()
        }

        // The prepass, shadow and deferred G-buffer passes run the same file
        // under `PREPASS_PIPELINE`, so a quad is expanded the same way in
        // every pass; see the module docs. Left at the default, the G-buffer
        // pass would run Bevy's mesh prepass shader on particle vertices.
        fn prepass_vertex_shader() -> ShaderRef {
            SHADER_3D.into()
        }

        fn prepass_fragment_shader() -> ShaderRef {
            SHADER_3D.into()
        }

        fn deferred_vertex_shader() -> ShaderRef {
            SHADER_3D.into()
        }

        fn deferred_fragment_shader() -> ShaderRef {
            SHADER_3D.into()
        }

        fn alpha_mode(&self) -> AlphaMode {
            alpha_mode_3d(self.blend, self.cutoff)
        }

        fn opaque_render_method(&self) -> bevy::material::OpaqueRendererMethod {
            self.route.opaque()
        }

        fn enable_oit() -> bool {
            false
        }

        fn specialize(
            _pipeline: &MaterialPipeline,
            descriptor: &mut RenderPipelineDescriptor,
            layout: &MeshVertexBufferLayoutRef,
            key: MaterialPipelineKey<Self>,
        ) -> Result<(), SpecializedMeshPipelineError> {
            apply_common(descriptor, layout, key.bind_group_data.mesh, true)
        }
    }
}

#[cfg(feature = "render_2d")]
pub use two::{alpha_mode_2d, blend_state_2d, Particle2dKey, ParticleMaterial2d};

#[cfg(feature = "render_2d")]
mod two {
    use super::*;
    use bevy::render::render_resource::{BlendComponent, BlendFactor, BlendOperation, BlendState};
    use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dKey, Material2dPipeline};

    #[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
    #[bind_group_data(Particle2dKey)]
    pub struct ParticleMaterial2d {
        #[uniform(0)]
        pub params: ParticleMaterialParams,
        #[texture(1)]
        #[sampler(2)]
        pub texture: Option<Handle<Image>>,
        pub blend: BlendMode,
        /// [`BlendMode::AlphaMask`]'s cutoff, for the pipeline's alpha mode.
        pub cutoff: f32,
    }

    /// Which blend state the 2D pipeline needs. Additive and premultiplied
    /// share one: both output premultiplied colour, and additive does it with
    /// alpha 0 so nothing behind is darkened. Multiply has its own.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct Particle2dKey {
        pub premultiplied: bool,
        pub multiply: bool,
    }

    impl Particle2dKey {
        pub fn of(blend: BlendMode) -> Self {
            Self {
                premultiplied: matches!(blend, BlendMode::Additive | BlendMode::Premultiplied),
                multiply: blend == BlendMode::Multiply,
            }
        }
    }

    impl From<&ParticleMaterial2d> for Particle2dKey {
        fn from(material: &ParticleMaterial2d) -> Self {
            Self::of(material.blend)
        }
    }

    /// The 2D pipeline's alpha mode. Opaque and masked particles go in the
    /// 2D opaque and alpha-mask phases, which write depth like the 3D ones;
    /// alpha to coverage is a mask here, since 2D has no coverage mode;
    /// everything else is blended, with the blend state fixed in `specialize`.
    pub fn alpha_mode_2d(blend: BlendMode, cutoff: f32) -> AlphaMode2d {
        match blend {
            BlendMode::Opaque => AlphaMode2d::Opaque,
            BlendMode::AlphaMask | BlendMode::AlphaToCoverage => {
                AlphaMode2d::Mask(finite_or_zero(cutoff).clamp(0.0, 1.0))
            }
            _ => AlphaMode2d::Blend,
        }
    }

    /// The blend state a blended 2D pipeline swaps in, `None` to keep Bevy's
    /// alpha blending. Multiply is Bevy's own 3D multiply state, `dst * src +
    /// dst * (1 - src.a)` on premultiplied colour, so a 2D soot layer darkens
    /// what is behind exactly as a 3D one does.
    pub fn blend_state_2d(key: Particle2dKey) -> Option<BlendState> {
        if key.multiply {
            return Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::Dst,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent::OVER,
            });
        }
        key.premultiplied.then_some(BlendState::PREMULTIPLIED_ALPHA_BLENDING)
    }

    /// Swap in [`blend_state_2d`] on every colour target.
    pub(crate) fn apply_blend_2d(descriptor: &mut RenderPipelineDescriptor, key: Particle2dKey) {
        let Some(state) = blend_state_2d(key) else { return };
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(state);
            }
        }
    }

    impl Material2d for ParticleMaterial2d {
        fn vertex_shader() -> ShaderRef {
            SHADER_2D.into()
        }

        fn fragment_shader() -> ShaderRef {
            SHADER_2D.into()
        }

        fn alpha_mode(&self) -> AlphaMode2d {
            alpha_mode_2d(self.blend, self.cutoff)
        }

        fn specialize(
            _pipeline: &Material2dPipeline,
            descriptor: &mut RenderPipelineDescriptor,
            layout: &MeshVertexBufferLayoutRef,
            key: Material2dKey<Self>,
        ) -> Result<(), SpecializedMeshPipelineError> {
            apply_common(descriptor, layout, false, false)?;
            apply_blend_2d(descriptor, key.bind_group_data);
            Ok(())
        }
    }
}

#[cfg(feature = "render_2d")]
pub(crate) use two::apply_blend_2d;
