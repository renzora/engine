//! The instanced grass pipeline.
//!
//! One draw call per chunk per foliage type: `VERTS_PER_BLADE` vertices by
//! however many blades that chunk scattered. The blade's geometry is rebuilt in
//! the vertex shader from `@builtin(vertex_index)`, so the only per-blade data
//! that crosses to the GPU is a [`GrassInstance`] in an instance-stepped vertex
//! buffer.
//!
//! Modelled on `renzora_grid::infinite_grid`, which is the tree's reference for
//! a hand-written render pipeline on this Bevy version. Two things differ:
//!
//! * **It lands in [`Opaque3d`], not `Transparent3d`.** Grass *is* opaque, and
//!   the transparent phase sorts per item by distance — a 64 m chunk whose
//!   centre sorts behind a particle that is actually inside it would draw in the
//!   wrong order. `Opaque3d` is binned rather than sorted, which is the correct
//!   home but means we own the bookkeeping; see [`EnqueuedGrass`].
//! * **It is double-sided** (`cull_mode: None`). A back-face-culled blade simply
//!   vanishes when you walk behind it, which is about half of them at any
//!   moment.
//!
//! # Not carried over from the old `Material` path
//!
//! The baked mesh was an ordinary Bevy `Material`, so it landed in the shadow
//! and depth-prepass pipelines for free. A hand-written pipeline does not:
//! **grass currently neither casts shadows nor writes into the depth prepass**,
//! so depth-driven effects (SSAO, contact shadows) see through it. Restoring
//! that means a second pipeline and queue system against the `Shadow` and
//! prepass phases running the same vertex expansion — deliberately left as a
//! follow-up rather than smuggled in here.

use bevy::camera::visibility::ViewVisibility;
use bevy::core_pipeline::core_3d::{
    Opaque3d, Opaque3dBatchSetKey, Opaque3dBinKey, CORE_3D_DEPTH_FORMAT,
};
use bevy::ecs::{
    prelude::*,
    query::ROQueryItem,
    system::{
        lifetimeless::{Read, SRes},
        SystemParamItem,
    },
};
use bevy::math::Vec4;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::render::render_resource::DynamicUniformBuffer;
use bevy::render::{
    camera::ExtractedCamera,
    mesh::{allocator::MeshSlabs, VertexBufferLayout},
    render_phase::{
        AddRenderCommand, BinnedRenderPhaseType, DrawFunctions, InputUniformIndex, PhaseItem,
        RenderCommand, RenderCommandResult, SetItemPipeline, TrackedRenderPass,
        ViewBinnedRenderPhases,
    },
    render_resource::{
        binding_types::uniform_buffer, BindGroup, BindGroupEntries, BindGroupLayoutDescriptor,
        BindGroupLayoutEntries, BufferUsages, ColorTargetState, ColorWrites, CompareFunction,
        DepthStencilState, FragmentState, MultisampleState, PipelineCache, PrimitiveState,
        RawBufferVec, RenderPipelineDescriptor, ShaderStages, ShaderType,
        SpecializedRenderPipeline, SpecializedRenderPipelines, TextureFormat, VertexFormat,
        VertexState, VertexStepMode,
    },
    renderer::{RenderDevice, RenderQueue},
    sync_world::{MainEntity, RenderEntity},
    view::{
        ExtractedView, RenderVisibleEntities, RetainedViewEntity, ViewUniform, ViewUniformOffset,
        ViewUniforms,
    },
    Extract, Render, RenderApp, RenderSystems,
};
use bevy::shader::Shader;

use super::instance::{BladeSetId, GrassChunk, GrassInstance, VERTS_PER_BLADE};

// Wind comes from the one world-global `renzora::WindState` (authored on the
// `WorldEnvironment` entity, evaluated by `renzora_wind`). This used to be a
// hardcoded `const WIND_DIRECTION: Vec2`, which meant grass could lean east
// under a cloud deck drifting west.

pub struct GrassRenderPlugin;

impl Plugin for GrassRenderPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "grass.wgsl");
    }

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .init_resource::<GrassPipeline>()
            .init_resource::<SpecializedRenderPipelines<GrassPipeline>>()
            .init_resource::<GrassParamsUniforms>()
            .init_resource::<GrassTime>()
            .init_resource::<EnqueuedGrass>()
            .add_render_command::<Opaque3d, DrawGrass>()
            .add_systems(ExtractSchedule, extract_grass_chunks)
            .add_systems(
                Render,
                (prepare_grass_instances, prepare_grass_uniforms)
                    .in_set(RenderSystems::PrepareResources),
            )
            .add_systems(
                Render,
                (prepare_grass_view_bind_groups, prepare_grass_bind_group)
                    .in_set(RenderSystems::PrepareBindGroups),
            )
            .add_systems(Render, queue_grass.in_set(RenderSystems::Queue));
    }
}

// ── Render-world data ───────────────────────────────────────────────────────

/// Per-chunk shader parameters. One dynamic-uniform slot per chunk.
#[derive(Clone, Copy, ShaderType)]
struct GrassParamsUniform {
    /// xyz = chunk origin in world space, w = this layer's wind strength.
    origin_wind: Vec4,
    /// xy = world wind direction, z = seconds, w = world wind strength
    /// (1.0 at `renzora::REFERENCE_WIND_SPEED`).
    wind_dir_time: Vec4,
    /// x = gust depth, y = gusts/sec, z = turbulence, w unused.
    wind_gust: Vec4,
    color_base: Vec4,
    color_tip: Vec4,
}

#[derive(Resource, Default)]
struct GrassParamsUniforms {
    buffer: DynamicUniformBuffer<GrassParamsUniform>,
}

/// The dynamic offset of this chunk's slot in [`GrassParamsUniforms`].
#[derive(Component)]
struct GrassParamsOffset(u32);

/// A chunk's blades, on the GPU.
///
/// Kept across frames and rewritten only when the blade set actually changes —
/// a chunk can hold a million instances, and re-uploading that every frame
/// because the component was extracted again would cost more than the old baked
/// mesh ever did. [`BladeSetId`] is what says whether it changed.
#[derive(Component)]
struct GrassInstanceBuffer {
    buffer: RawBufferVec<GrassInstance>,
    uploaded: Option<BladeSetId>,
}

/// Which grass entities are currently sitting in each view's opaque bin.
///
/// [`Opaque3d`] is a *binned* phase: bins persist between frames and Bevy only
/// sweeps the entities it owns itself. A `NonMesh` item is ours, so a chunk that
/// stops being queued — repainted, culled, despawned — would otherwise sit in
/// the bin forever, drawing stale grass and leaking a slot per repaint. This is
/// the previous frame's enqueued set, diffed against the current one so those
/// entities get explicitly removed.
#[derive(Resource, Default)]
struct EnqueuedGrass {
    per_view: HashMap<RetainedViewEntity, HashSet<MainEntity>>,
}

// ── Extract ─────────────────────────────────────────────────────────────────

/// Pull visible grass chunks into the render world.
///
/// A hidden or emptied chunk is **actively removed**, not merely skipped: render
/// entities are retained between frames, so last frame's blades would otherwise
/// keep drawing after the chunk was repainted to bare ground.
fn extract_grass_chunks(
    mut commands: Commands,
    time: Extract<Res<Time>>,
    wind: Extract<Option<Res<renzora::WindState>>>,
    chunks: Extract<
        Query<(
            RenderEntity,
            &GrassChunk,
            &GlobalTransform,
            Option<&ViewVisibility>,
        )>,
    >,
    mut grass_time: ResMut<GrassTime>,
) {
    grass_time.seconds = time.elapsed_secs();
    // Absent when the wind plugin is stripped from a lean export — dead calm
    // is the correct fallback, not a panic.
    grass_time.wind = wind.as_deref().copied().unwrap_or_default();
    for (render_entity, chunk, transform, visibility) in chunks.iter() {
        let visible = visibility.is_none_or(|v| v.get());
        if !visible || chunk.is_empty() {
            commands
                .entity(render_entity)
                .remove::<(GrassChunk, ExtractedGrassOrigin)>();
            continue;
        }
        commands
            .entity(render_entity)
            .insert((chunk.clone(), ExtractedGrassOrigin(transform.translation())));
    }
}

/// The chunk's world origin. Blade positions are chunk-local, so the shader adds
/// this — there is no mesh uniform on this path to carry a model matrix.
#[derive(Component)]
struct ExtractedGrassOrigin(Vec3);

/// Seconds plus the world wind, extracted once per frame rather than read per
/// chunk. `WindState` is `Copy`, so this is a plain snapshot — the render world
/// never touches the main-world resource.
#[derive(Resource, Default)]
pub(super) struct GrassTime {
    seconds: f32,
    wind: renzora::WindState,
}

// ── Prepare ─────────────────────────────────────────────────────────────────

fn prepare_grass_instances(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut chunks: Query<(Entity, &GrassChunk, Option<&mut GrassInstanceBuffer>)>,
) {
    for (entity, chunk, existing) in chunks.iter_mut() {
        match existing {
            // Already on the GPU and unchanged — by far the common case, since
            // a chunk is rescattered only when it is repainted.
            Some(gpu) if gpu.uploaded == Some(chunk.id) => {}
            Some(mut gpu) => {
                fill(&mut gpu.buffer, chunk);
                gpu.buffer.write_buffer(&render_device, &render_queue);
                gpu.uploaded = Some(chunk.id);
            }
            None => {
                let mut buffer = RawBufferVec::<GrassInstance>::new(
                    BufferUsages::VERTEX | BufferUsages::COPY_DST,
                );
                fill(&mut buffer, chunk);
                buffer.write_buffer(&render_device, &render_queue);
                commands.entity(entity).insert(GrassInstanceBuffer {
                    buffer,
                    uploaded: Some(chunk.id),
                });
            }
        }
    }

    fn fill(buffer: &mut RawBufferVec<GrassInstance>, chunk: &GrassChunk) {
        buffer.clear();
        buffer.reserve_internal(chunk.len());
        for blade in chunk.blades.iter() {
            buffer.push(*blade);
        }
    }
}

fn prepare_grass_uniforms(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut uniforms: ResMut<GrassParamsUniforms>,
    time: Res<GrassTime>,
    chunks: Query<(Entity, &GrassChunk, &ExtractedGrassOrigin)>,
) {
    let Some(mut writer) =
        uniforms
            .buffer
            .get_writer(chunks.iter().len(), &render_device, &render_queue)
    else {
        return;
    };
    let wind = time.wind;
    for (entity, chunk, origin) in chunks.iter() {
        let offset = writer.write(&GrassParamsUniform {
            origin_wind: origin.0.extend(chunk.wind_strength),
            wind_dir_time: Vec4::new(
                wind.direction.x,
                wind.direction.y,
                time.seconds,
                wind.strength01(),
            ),
            wind_gust: Vec4::new(
                wind.gust_strength,
                wind.gust_frequency,
                wind.turbulence,
                0.0,
            ),
            color_base: Vec4::new(
                chunk.color_base.red,
                chunk.color_base.green,
                chunk.color_base.blue,
                1.0,
            ),
            color_tip: Vec4::new(
                chunk.color_tip.red,
                chunk.color_tip.green,
                chunk.color_tip.blue,
                1.0,
            ),
        });
        commands.entity(entity).insert(GrassParamsOffset(offset));
    }
}

#[derive(Resource)]
struct GrassParamsBindGroup(BindGroup);

#[derive(Component)]
struct GrassViewBindGroup(BindGroup);

fn prepare_grass_view_bind_groups(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipeline: Res<GrassPipeline>,
    pipeline_cache: Res<PipelineCache>,
    view_uniforms: Res<ViewUniforms>,
    views: Query<Entity, With<ViewUniformOffset>>,
) {
    let Some(binding) = view_uniforms.uniforms.binding() else {
        return;
    };
    for entity in views.iter() {
        let bind_group = render_device.create_bind_group(
            "grass_view_bind_group",
            &pipeline_cache.get_bind_group_layout(&pipeline.view_layout),
            &BindGroupEntries::single(binding.clone()),
        );
        commands
            .entity(entity)
            .insert(GrassViewBindGroup(bind_group));
    }
}

fn prepare_grass_bind_group(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipeline: Res<GrassPipeline>,
    pipeline_cache: Res<PipelineCache>,
    uniforms: Res<GrassParamsUniforms>,
) {
    let Some(binding) = uniforms.buffer.binding() else {
        return;
    };
    let bind_group = render_device.create_bind_group(
        "grass_params_bind_group",
        &pipeline_cache.get_bind_group_layout(&pipeline.params_layout),
        &BindGroupEntries::single(binding),
    );
    commands.insert_resource(GrassParamsBindGroup(bind_group));
}

// ── Queue ───────────────────────────────────────────────────────────────────

fn queue_grass(
    pipeline_cache: Res<PipelineCache>,
    draw_functions: Res<DrawFunctions<Opaque3d>>,
    pipeline: Res<GrassPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<GrassPipeline>>,
    mut opaque_phases: ResMut<ViewBinnedRenderPhases<Opaque3d>>,
    mut enqueued: ResMut<EnqueuedGrass>,
    chunks: Query<Entity, (With<GrassChunk>, With<GrassInstanceBuffer>)>,
    views: Query<(&ExtractedView, &RenderVisibleEntities, &Msaa), With<ExtractedCamera>>,
) {
    let Some(draw_function) = draw_functions.read().get_id::<DrawGrass>() else {
        return;
    };

    for (view, visible, msaa) in views.iter() {
        let Some(phase) = opaque_phases.get_mut(&view.retained_view_entity) else {
            continue;
        };
        let pipeline_id = pipelines.specialize(
            &pipeline_cache,
            &pipeline,
            GrassPipelineKey {
                target_format: view.target_format,
                sample_count: msaa.samples(),
            },
        );

        let mut now = HashSet::default();
        if let Some(visible_grass) = visible.get::<GrassChunk>() {
            for (render_entity, main_entity) in visible_grass.iter_visible() {
                if chunks.get(*render_entity).is_err() {
                    continue;
                }
                phase.add(
                    Opaque3dBatchSetKey {
                        pipeline: pipeline_id,
                        draw_function,
                        material_bind_group_index: None,
                        // No mesh, so no slab to name. Every grass item shares
                        // this, which is fine: `NonMesh` items are drawn one
                        // after another with no batching attempted.
                        slabs: MeshSlabs::default(),
                        lightmap_slab: None,
                    },
                    Opaque3dBinKey {
                        asset_id: pipeline.shader.id().untyped(),
                    },
                    (*render_entity, *main_entity),
                    InputUniformIndex::default(),
                    BinnedRenderPhaseType::NonMesh,
                );
                now.insert(*main_entity);
            }
        }

        // Anything that was in the bin last frame and isn't now has to be taken
        // out by hand — see `EnqueuedGrass`.
        if let Some(previous) = enqueued.per_view.get(&view.retained_view_entity) {
            for stale in previous.difference(&now) {
                phase.remove(*stale);
            }
        }
        enqueued.per_view.insert(view.retained_view_entity, now);
    }
}

// ── Draw ────────────────────────────────────────────────────────────────────

type DrawGrass = (SetItemPipeline, SetGrassBindGroups, DrawGrassInstanced);

struct SetGrassBindGroups;

impl<P: PhaseItem> RenderCommand<P> for SetGrassBindGroups {
    type Param = SRes<GrassParamsBindGroup>;
    type ViewQuery = (Read<ViewUniformOffset>, Read<GrassViewBindGroup>);
    type ItemQuery = Read<GrassParamsOffset>;

    #[inline]
    fn render<'w>(
        _item: &P,
        (view_uniform, view_bind_group): ROQueryItem<'w, '_, Self::ViewQuery>,
        params_offset: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        params_bind_group: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some(params_offset) = params_offset else {
            return RenderCommandResult::Skip;
        };
        pass.set_bind_group(0, &view_bind_group.0, &[view_uniform.offset]);
        pass.set_bind_group(1, &params_bind_group.into_inner().0, &[params_offset.0]);
        RenderCommandResult::Success
    }
}

struct DrawGrassInstanced;

impl<P: PhaseItem> RenderCommand<P> for DrawGrassInstanced {
    type Param = ();
    type ViewQuery = ();
    type ItemQuery = Read<GrassInstanceBuffer>;

    #[inline]
    fn render<'w>(
        _item: &P,
        _view: ROQueryItem<'w, '_, Self::ViewQuery>,
        instances: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        _param: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        // A bin entry can outlive its buffer by a frame — the entity is removed
        // in `queue_grass`, which runs after the phase was already populated.
        // Skipping is the correct response, not an error.
        let Some(instances) = instances else {
            return RenderCommandResult::Skip;
        };
        let Some(buffer) = instances.buffer.buffer() else {
            return RenderCommandResult::Skip;
        };
        let count = instances.buffer.len() as u32;
        if count == 0 {
            return RenderCommandResult::Skip;
        }
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.draw(0..VERTS_PER_BLADE, 0..count);
        RenderCommandResult::Success
    }
}

// ── Pipeline ────────────────────────────────────────────────────────────────

#[derive(Resource)]
struct GrassPipeline {
    view_layout: BindGroupLayoutDescriptor,
    params_layout: BindGroupLayoutDescriptor,
    shader: Handle<Shader>,
}

impl FromWorld for GrassPipeline {
    fn from_world(world: &mut World) -> Self {
        let view_layout = BindGroupLayoutDescriptor::new(
            "grass_view_bind_group_layout",
            &BindGroupLayoutEntries::single(
                ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                uniform_buffer::<ViewUniform>(true),
            ),
        );
        let params_layout = BindGroupLayoutDescriptor::new(
            "grass_params_bind_group_layout",
            &BindGroupLayoutEntries::single(
                ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                uniform_buffer::<GrassParamsUniform>(true),
            ),
        );
        let shader =
            bevy::asset::load_embedded_asset!(world.resource::<AssetServer>(), "grass.wgsl");
        Self {
            view_layout,
            params_layout,
            shader,
        }
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
struct GrassPipelineKey {
    target_format: TextureFormat,
    sample_count: u32,
}

impl SpecializedRenderPipeline for GrassPipeline {
    type Key = GrassPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("grass_render_pipeline".into()),
            layout: vec![self.view_layout.clone(), self.params_layout.clone()],
            vertex: VertexState {
                shader: self.shader.clone(),
                entry_point: Some("vertex".into()),
                // One instance-stepped buffer holding `GrassInstance`; the blade
                // itself has no vertex buffer at all.
                buffers: vec![VertexBufferLayout::from_vertex_formats(
                    VertexStepMode::Instance,
                    [
                        VertexFormat::Float32x4,
                        VertexFormat::Float32x4,
                        VertexFormat::Float32x4,
                    ],
                )],
                ..Default::default()
            },
            primitive: PrimitiveState {
                // Double-sided: a culled blade disappears when viewed from
                // behind, and the fragment shader flips the normal instead.
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                // Reverse-Z, matching the rest of the 3D pipeline.
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: MultisampleState {
                count: key.sample_count,
                ..Default::default()
            },
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format: key.target_format,
                    // Opaque — no blending, and it writes depth.
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                ..Default::default()
            }),
            ..Default::default()
        }
    }
}
