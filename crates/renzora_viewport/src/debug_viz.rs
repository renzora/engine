//! Post-tonemap debug visualization for normals + linear depth.
//!
//! The existing material-swap viz (`viewport_debug.wesl`) renders into
//! the HDR view target before tonemapping, so its colors get squashed by
//! ACES tonemap + auto-exposure and end up looking dim/washed — that's
//! the "their normals look nothing like Solari's" issue.
//!
//! This pass runs *after* the Tonemapping render-graph node, reads the
//! prepass textures directly, and writes display-ready colors straight
//! to the view target. Bypasses tonemap, AE, and any post-tonemap
//! effects. Used for Normals + Depth (data already in prepass).
//! Roughness/Metallic/UV Checker stay on the existing material-swap
//! path because they need per-fragment material data.

use bevy::core_pipeline::Core3d;
use bevy::core_pipeline::prepass::{DepthPrepass, NormalPrepass, ViewPrepassTextures};
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::sync_component::SyncComponent;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::render_resource::binding_types::{
    sampler, texture_2d, texture_depth_2d, uniform_buffer,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy::render::view::{ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms};
use bevy::render::{Render, RenderApp, RenderSystems};
use bytemuck::{Pod, Zeroable};

use renzora::core::viewport_types::{ViewportSettings, VisualizationMode};

/// Marker mirrored to view entities each frame; the value selects which
/// post-tonemap visualization to run. None = pass through unchanged.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct DebugVizMode {
    pub mode: VisualizationMode,
}

// `ExtractComponent` requires the `SyncComponent` supertrait; both name the
// target app since 0.20.
impl SyncComponent<bevy::render::RenderApp> for DebugVizMode {
    type Target = Self;
}

impl ExtractComponent<bevy::render::RenderApp> for DebugVizMode {
    type QueryData = &'static DebugVizMode;
    type QueryFilter = ();
    type Out = DebugVizMode;

    fn extract_component(item: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(*item)
    }
}

#[derive(Clone, Copy, Debug, Pod, Zeroable, ShaderType)]
#[repr(C)]
struct DebugVizConfig {
    mode: u32,
    near: f32,
    far: f32,
    _pad: f32,
}

#[derive(Resource)]
pub struct DebugVizPipeline {
    layout: BindGroupLayoutDescriptor,
    pipeline_id: CachedRenderPipelineId,
    sampler: Sampler,
}

#[derive(Component)]
pub struct DebugVizResources {
    config_buffer: Buffer,
    /// The mode last written into `config_buffer`, so the buffer is only
    /// rewritten when it changes.
    written_mode: u32,
}


impl FromWorld for DebugVizPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = BindGroupLayoutDescriptor::new(
            "debug_viz_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    texture_depth_2d(),
                    texture_2d(TextureSampleType::Float { filterable: false }),
                    uniform_buffer::<ViewUniform>(true),
                    uniform_buffer::<DebugVizConfig>(false),
                ),
            ),
        );

        let sampler = render_device.create_sampler(&SamplerDescriptor::default());

        let asset_server = world.resource::<AssetServer>();
        let shader = asset_server.load("embedded://renzora_viewport/debug_viz.wesl");

        let fullscreen = world.resource::<bevy::core_pipeline::FullscreenShader>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some("debug_viz_pipeline".into()),
            layout: vec![layout.clone()],
            vertex: fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader,
                shader_defs: vec![],
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    // 0.19: TEXTURE_FORMAT_HDR deprecated.
                    format: TextureFormat::Rgba16Float,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                constants: vec![],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            immediate_size: 0,
            zero_initialize_workgroup_memory: false,
        });

        Self {
            layout,
            pipeline_id,
            sampler,
        }
    }
}

/// Mirror the global `ViewportSettings.visualization_mode` onto each camera
/// as a `DebugVizMode` component. Also ensures the prepass components are
/// attached when a viz mode that needs them is active — except those are
/// already permanently attached at editor camera spawn so this is a no-op
/// in practice today.
pub fn sync_debug_viz_mode(
    mut commands: Commands,
    settings: Option<Res<ViewportSettings>>,
    cameras: Query<(Entity, Option<&DebugVizMode>), With<Camera3d>>,
) {
    let mode = settings.map(|s| s.visualization_mode).unwrap_or_default();
    for (entity, current) in &cameras {
        // Inserting unconditionally re-flagged the component on every camera
        // every frame, for a value that almost never changes.
        if current.is_some_and(|c| c.mode == mode) {
            continue;
        }
        // try_insert: a camera can be despawned before this end-of-schedule
        // command applies (e.g. opening a document tab tears down the scene
        // camera). `insert` would panic on the dead entity.
        commands.entity(entity).try_insert(DebugVizMode { mode });
    }
}

/// Create each view's config buffer once and rewrite it only when the mode
/// changes.
///
/// This used to `write_buffer` 16 bytes per camera on every frame. Each write
/// takes the render queue's lock, and while other prepare systems were creating
/// bind groups that lock was contended badly enough that this system never ran
/// in under 3.4 ms, for a value that changes when someone picks a different
/// visualization mode.
pub fn prepare_debug_viz_uniforms(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut views: Query<(Entity, &DebugVizMode, Option<&mut DebugVizResources>)>,
) {
    for (entity, mode, existing) in &mut views {
        let mode_u32 = match mode.mode {
            VisualizationMode::Normals => 1,
            VisualizationMode::Depth => 2,
            _ => 0,
        };
        let config = DebugVizConfig {
            mode: mode_u32,
            near: 0.1,
            far: 200.0,
            _pad: 0.0,
        };

        match existing {
            Some(mut res) => {
                if res.written_mode != mode_u32 {
                    render_queue.write_buffer(&res.config_buffer, 0, bytemuck::bytes_of(&config));
                    res.written_mode = mode_u32;
                }
            }
            None => {
                let buffer = render_device.create_buffer(&BufferDescriptor {
                    label: Some("debug_viz_config_buffer"),
                    size: std::mem::size_of::<DebugVizConfig>() as u64,
                    usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                render_queue.write_buffer(&buffer, 0, bytemuck::bytes_of(&config));
                commands.entity(entity).insert(DebugVizResources {
                    config_buffer: buffer,
                    written_mode: mode_u32,
                });
            }
        }
    }
}

/// Debug visualization pass (normals / depth). Bevy 0.19: was
/// `DebugVizNode: ViewNode`; now a render system in the `Core3d` schedule.
pub fn debug_viz_pass(
    world: &World,
    view: ViewQuery<(
        &'static ViewTarget,
        &'static ViewUniformOffset,
        &'static ViewPrepassTextures,
        &'static DebugVizResources,
        &'static DebugVizMode,
    )>,
    mut render_context: RenderContext,
) {
    let (view_target, view_offset, prepass, resources, mode) = view.into_inner();

    // Mode None: nothing to do, scene passes through normally.
    if !matches!(
        mode.mode,
        VisualizationMode::Normals | VisualizationMode::Depth
    ) {
        return;
    }

    let pipeline = world.resource::<DebugVizPipeline>();
    let pipeline_cache = world.resource::<PipelineCache>();
    let view_uniforms = world.resource::<ViewUniforms>();

    let Some(render_pipeline) = pipeline_cache.get_render_pipeline(pipeline.pipeline_id) else {
        return;
    };
    let Some(view_binding) = view_uniforms.uniforms.binding() else {
        return;
    };
    let Some(depth_view) = prepass.depth_only_view() else {
        return;
    };
    let Some(normal_view) = prepass.normal_view() else {
        return;
    };

    let post_process = view_target.post_process_write();

    let bind_group = render_context.render_device().create_bind_group(
        "debug_viz_bind_group",
        &pipeline_cache.get_bind_group_layout(&pipeline.layout),
        &BindGroupEntries::sequential((
            post_process.source,
            &pipeline.sampler,
            depth_view,
            normal_view,
            view_binding.clone(),
            resources.config_buffer.as_entire_binding(),
        )),
    );

    let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("debug_viz_pass"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: post_process.destination,
            depth_slice: None,
            resolve_target: None,
            ops: Operations::default(),
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });

    render_pass.set_render_pipeline(render_pipeline);
    render_pass.set_bind_group(0, &bind_group, &[view_offset.offset]);
    render_pass.draw(0..3, 0..1);
}

#[derive(Default)]
pub struct DebugVizPlugin;

impl Plugin for DebugVizPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "debug_viz.wesl");

        app.add_systems(Update, sync_debug_viz_mode);
        app.add_plugins(ExtractComponentPlugin::<DebugVizMode>::default());

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .add_systems(
                    Render,
                    prepare_debug_viz_uniforms.in_set(RenderSystems::PrepareResources),
                )
                // A debug visualization drawn last → the shared `RenderPhase::Overlay`
                // (after tonemapping + AA; the framework owns the placement).
                .add_systems(
                    Core3d,
                    debug_viz_pass.in_set(renzora::RenderPhase::Overlay),
                );
        }
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<DebugVizPipeline>();
        }
    }
}

// Silence unused-import warnings if the prepass markers aren't referenced
// elsewhere in this file (we only need them for the doc comment context).
#[allow(dead_code)]
fn _prepass_imports_used(_depth: DepthPrepass, _normal: NormalPrepass) {}
