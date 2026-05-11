//! Phase 6 — Lumen voxel-cache GI trace with temporal accumulation.
//!
//! Runs as a fragment-shader fullscreen pass after the voxel resolve
//! and before tonemapping, additively layering off-screen indirect
//! contribution onto the HDR scene. Active when the camera's
//! `LumenLighting.quality` is `SdfLow` or above.
//!
//! Uses ping-pong history textures (RGB = blended indirect, A = linear
//! view-space depth at write time) with motion-vector reprojection +
//! depth disocclusion to absorb per-frame trace noise. Pattern mirrors
//! `renzora_rt`'s SSGI temporal accumulator.

use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::core_pipeline::prepass::ViewPrepassTextures;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::render_graph::{
    NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
};
use bevy::render::render_resource::binding_types::{
    sampler, texture_2d, texture_3d, texture_depth_2d, uniform_buffer,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue};
use bevy::render::view::{ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms};
use bevy::render::{Render, RenderApp, RenderSystems};
use bevy::utils::default;
use bytemuck::{Pod, Zeroable};
use std::sync::atomic::{AtomicU32, Ordering};

use crate::voxel_cache::{
    VoxelCacheResources, VoxelCacheView, VoxelGridUniform, VoxelResolveLabel,
};
use crate::LumenLighting;

#[derive(Clone, Copy, Debug, Pod, Zeroable, ShaderType)]
#[repr(C)]
pub struct TraceConfig {
    pub intensity: f32,
    pub frame_count: u32,
    /// 0 = composite (scene + indirect). 1 = indirect-only (debug —
    /// shows just the trace contribution). Driven by
    /// `LumenLighting.debug == LumenDebug::IndirectOnly`.
    pub debug_mode: u32,
    pub _pad0: u32,
}

#[derive(Resource)]
pub struct LumenTracePipeline {
    pub layout: BindGroupLayoutDescriptor,
    pub pipeline_id: CachedRenderPipelineId,
    pub voxel_sampler: Sampler,
    pub scene_sampler: Sampler,
    pub frame_count: AtomicU32,
}

#[derive(Component)]
pub struct LumenTraceResources {
    pub config_buffer: Buffer,
    /// Ping-pong history buffers. Each frame one is read (last frame's
    /// indirect + linear depth in alpha) and the other is written.
    pub history_a: TextureView,
    pub history_b: TextureView,
    /// Cached size to detect view-target resizes and recreate history.
    pub history_size: Extent3d,
}

impl Clone for LumenTraceResources {
    fn clone(&self) -> Self {
        Self {
            config_buffer: self.config_buffer.clone(),
            history_a: self.history_a.clone(),
            history_b: self.history_b.clone(),
            history_size: self.history_size,
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct LumenTraceLabel;

impl FromWorld for LumenTracePipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = BindGroupLayoutDescriptor::new(
            "lumen_trace_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    // 0: scene HDR (post-process source)
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    // 1: scene sampler
                    sampler(SamplerBindingType::Filtering),
                    // 2: depth prepass
                    texture_depth_2d(),
                    // 3: normal prepass
                    texture_2d(TextureSampleType::Float { filterable: false }),
                    // 4: voxel radiance (clipmap mega-texture)
                    texture_3d(TextureSampleType::Float { filterable: true }),
                    // 5: voxel sampler
                    sampler(SamplerBindingType::Filtering),
                    // 6: previous-frame history (RGB = indirect, A = linear depth)
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    // 7: motion vectors prepass
                    texture_2d(TextureSampleType::Float { filterable: false }),
                    // 8: view uniform (dynamic offset)
                    uniform_buffer::<ViewUniform>(true),
                    // 9: voxel grid uniform
                    uniform_buffer::<VoxelGridUniform>(false),
                    // 10: trace config
                    uniform_buffer::<TraceConfig>(false),
                ),
            ),
        );

        let scene_sampler = render_device.create_sampler(&SamplerDescriptor {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..default()
        });
        let voxel_sampler = render_device.create_sampler(&SamplerDescriptor::default());

        let asset_server = world.resource::<AssetServer>();
        let shader = asset_server.load("embedded://renzora_lumen/lumen_trace.wgsl");
        let fullscreen = world.resource::<bevy::core_pipeline::FullscreenShader>();

        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some("lumen_trace_pipeline".into()),
            layout: vec![layout.clone()],
            vertex: fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader,
                shader_defs: vec![],
                entry_point: Some("fragment".into()),
                targets: vec![
                    // Target 0: composited scene + indirect → view target.
                    Some(ColorTargetState {
                        format: ViewTarget::TEXTURE_FORMAT_HDR,
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    }),
                    // Target 1: new history (indirect.rgb, linear_depth).
                    Some(ColorTargetState {
                        format: TextureFormat::Rgba16Float,
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    }),
                ],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            push_constant_ranges: vec![],
            zero_initialize_workgroup_memory: false,
        });

        Self {
            layout,
            pipeline_id,
            voxel_sampler,
            scene_sampler,
            frame_count: AtomicU32::new(0),
        }
    }
}

pub fn prepare_lumen_trace_uniforms(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    pipeline: Res<LumenTracePipeline>,
    views: Query<
        (Entity, &LumenLighting, &ViewTarget, Option<&LumenTraceResources>),
        With<VoxelCacheView>,
    >,
) {
    let frame = pipeline.frame_count.fetch_add(1, Ordering::Relaxed);
    for (entity, lighting, view_target, existing) in &views {
        let debug_mode = match lighting.debug {
            crate::LumenDebug::IndirectOnly => 1,
            _ => 0,
        };
        let config = TraceConfig {
            intensity: lighting.intensity,
            frame_count: frame,
            debug_mode,
            _pad0: 0,
        };

        let target_size = view_target.main_texture().size();
        let needs_alloc = match existing {
            Some(res) => res.history_size != target_size,
            None => true,
        };

        let resources = if needs_alloc {
            let history_a = render_device.create_texture(&TextureDescriptor {
                label: Some("lumen_trace_history_a"),
                size: target_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba16Float,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let history_b = render_device.create_texture(&TextureDescriptor {
                label: Some("lumen_trace_history_b"),
                size: target_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba16Float,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let config_buffer = if let Some(prev) = existing {
                prev.config_buffer.clone()
            } else {
                render_device.create_buffer(&BufferDescriptor {
                    label: Some("lumen_trace_config"),
                    size: std::mem::size_of::<TraceConfig>() as u64,
                    usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            };
            let res = LumenTraceResources {
                config_buffer,
                history_a: history_a.create_view(&TextureViewDescriptor::default()),
                history_b: history_b.create_view(&TextureViewDescriptor::default()),
                history_size: target_size,
            };
            commands.entity(entity).insert(res.clone());
            res
        } else {
            existing.unwrap().clone()
        };

        render_queue.write_buffer(&resources.config_buffer, 0, bytemuck::bytes_of(&config));
    }
}

#[derive(Default)]
pub struct LumenTraceNode;

impl ViewNode for LumenTraceNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static ViewUniformOffset,
        &'static ViewPrepassTextures,
        &'static VoxelCacheView,
        &'static LumenTraceResources,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, view_offset, prepass, view, resources): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        // inject_active is true exactly when LumenQuality >= SdfLow,
        // which is also when the cache is being maintained and we
        // want this trace running.
        if !view.inject_active { return Ok(()); }

        let pipeline = world.resource::<LumenTracePipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let view_uniforms = world.resource::<ViewUniforms>();

        let Some(render_pipeline) = pipeline_cache.get_render_pipeline(pipeline.pipeline_id)
        else { return Ok(()); };
        let Some(view_binding) = view_uniforms.uniforms.binding() else { return Ok(()); };
        let Some(depth_view) = prepass.depth_view() else { return Ok(()); };
        let Some(normal_view) = prepass.normal_view() else { return Ok(()); };
        let Some(motion_view) = prepass.motion_vectors_view() else { return Ok(()); };
        let Some(cache_res) = world.get_resource::<VoxelCacheResources>() else {
            return Ok(());
        };

        // Frame-count parity decides which history texture is "previous"
        // (read) and which is "next" (written this frame). The prepare
        // system incremented frame_count before stamping the uniform, so
        // frame_count - 1 matches what `config.frame_count` holds.
        let frame = pipeline.frame_count.load(Ordering::Relaxed).wrapping_sub(1);
        let (read_history, write_history) = if frame % 2 == 0 {
            (&resources.history_a, &resources.history_b)
        } else {
            (&resources.history_b, &resources.history_a)
        };

        let post_process = view_target.post_process_write();

        let bind_group = render_context.render_device().create_bind_group(
            "lumen_trace_bind_group",
            &pipeline_cache.get_bind_group_layout(&pipeline.layout),
            &BindGroupEntries::sequential((
                post_process.source,
                &pipeline.scene_sampler,
                depth_view,
                normal_view,
                &cache_res.radiance,
                &pipeline.voxel_sampler,
                read_history,
                motion_view,
                view_binding.clone(),
                cache_res.uniform_buffer.as_entire_binding(),
                resources.config_buffer.as_entire_binding(),
            )),
        );

        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("lumen_trace_pass"),
            color_attachments: &[
                // Target 0: composite → view target dest.
                Some(RenderPassColorAttachment {
                    view: post_process.destination,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations::default(),
                }),
                // Target 1: new history (blended indirect, linear depth).
                Some(RenderPassColorAttachment {
                    view: write_history,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations::default(),
                }),
            ],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render_pass.set_render_pipeline(render_pipeline);
        render_pass.set_bind_group(0, &bind_group, &[view_offset.offset]);
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}

#[derive(Default)]
pub struct LumenTracePlugin;

impl Plugin for LumenTracePlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "lumen_trace.wgsl");

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .add_systems(
                    Render,
                    prepare_lumen_trace_uniforms.in_set(RenderSystems::PrepareResources),
                )
                .add_render_graph_node::<ViewNodeRunner<LumenTraceNode>>(Core3d, LumenTraceLabel)
                // Runs after the voxel cache is resolved (so we read fresh
                // data this frame) and before tonemapping so output lands
                // in HDR scene space.
                .add_render_graph_edges(
                    Core3d,
                    (VoxelResolveLabel, LumenTraceLabel, Node3d::Tonemapping),
                );
        }
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<LumenTracePipeline>();
        }
    }
}
