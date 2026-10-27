//! The debug view: what the viewport's Visualization menu calls "Virtual
//! Geometry: Clusters" and friends. See `shaders/debug.wesl`.
//!
//! Drawn on the editor's viewport cameras only, so a game
//! preview, a thumbnail or the Play window is never tinted by it.

use std::collections::{HashMap, HashSet};

use bevy::core_pipeline::prepass::ViewPrepassTextures;
use bevy::prelude::*;
use bevy::render::render_resource::binding_types::{texture_2d, texture_depth_2d, uniform_buffer, uniform_buffer_sized};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::sync_world::RenderEntity;
use bevy::render::view::{ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms};
use bevy::render::Extract;
use renzora::core::viewport_types::{ViewportSettings, VisualizationMode};
use renzora::EditorCamera;

use super::pipelines::VgPipelines;
use super::view::{VgView, VgViewBindGroups};

/// Which debug view is on, and the editor cameras it applies to.
#[derive(Resource, Default)]
pub(crate) struct VgDebug {
    mode: u32,
    views: HashSet<Entity>,
}

pub(crate) fn extract(
    mut debug: ResMut<VgDebug>,
    settings: Extract<Option<Res<ViewportSettings>>>,
    cameras: Extract<Query<&RenderEntity, With<EditorCamera>>>,
) {
    debug.mode = match settings.as_ref().map(|s| s.visualization_mode) {
        Some(VisualizationMode::VgClusters) => 1,
        Some(VisualizationMode::VgTriangles) => 2,
        Some(VisualizationMode::VgLod) => 3,
        Some(VisualizationMode::VgInstances) => 4,
        Some(VisualizationMode::VgResidency) => 5,
        _ => 0,
    };
    debug.views.clear();
    if debug.mode != 0 {
        debug.views.extend(cameras.iter().map(|e| e.id()));
    }
}

#[derive(Resource)]
pub(crate) struct DebugPipelines {
    layout: BindGroupLayoutDescriptor,
    layout_with_depth: BindGroupLayoutDescriptor,
    shader: Handle<Shader>,
    specialized: HashMap<(TextureFormat, bool), CachedRenderPipelineId>,
}

pub(crate) fn init(mut commands: Commands, asset_server: Res<AssetServer>) {
    let entries = |depth: bool| {
        let mut entries = BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (
                uniform_buffer::<ViewUniform>(true),
                texture_2d(TextureSampleType::Float { filterable: false }),
                uniform_buffer_sized(false, std::num::NonZeroU64::new(16)),
            ),
        )
        .to_vec();
        if depth {
            entries.extend(BindGroupLayoutEntries::with_indices(
                ShaderStages::FRAGMENT,
                ((3, texture_depth_2d()),),
            ).iter().copied());
        }
        entries
    };
    commands.insert_resource(DebugPipelines {
        layout: BindGroupLayoutDescriptor::new("vg_debug_layout", &entries(false)),
        layout_with_depth: BindGroupLayoutDescriptor::new("vg_debug_layout_depth", &entries(true)),
        shader: asset_server.load("embedded://renzora_virtual_geometry/shaders/debug.wesl"),
        specialized: HashMap::new(),
    });
}

impl DebugPipelines {
    #[cfg(test)]
    pub(crate) fn ids(&self) -> Vec<CachedRenderPipelineId> {
        self.specialized.values().copied().collect()
    }
}

/// The pipeline a view's debug pass uses: one per target format, with or
/// without the depth test.
#[derive(Component)]
pub(crate) struct VgDebugPipeline {
    id: CachedRenderPipelineId,
    depth: bool,
}

pub(crate) fn prepare(
    mut commands: Commands,
    debug: Res<VgDebug>,
    mut pipelines: ResMut<DebugPipelines>,
    vg: Option<Res<VgPipelines>>,
    pipeline_cache: Res<PipelineCache>,
    views: Query<(Entity, &ViewTarget, Option<&ViewPrepassTextures>), With<VgView>>,
) {
    let Some(vg) = vg else {
        return;
    };
    for (entity, target, prepass) in &views {
        if !debug.views.contains(&entity) {
            commands.entity(entity).remove::<VgDebugPipeline>();
            continue;
        }
        let depth = prepass.is_some_and(|p| p.depth_only_view().is_some());
        let format = target.main_texture_format();
        let pipelines = &mut *pipelines;
        let id = *pipelines.specialized.entry((format, depth)).or_insert_with(|| {
            let mut defs = super::pipelines::vg_bindings(0, 0).to_vec();
            if depth {
                defs.push("DEPTH_TEST".into());
            }
            pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
                label: Some("vg_debug".into()),
                layout: vec![
                    vg.shade_layout.clone(),
                    if depth { pipelines.layout_with_depth.clone() } else { pipelines.layout.clone() },
                ],
                vertex: VertexState {
                    shader: pipelines.shader.clone(),
                    shader_defs: defs.clone(),
                    ..default()
                },
                fragment: Some(FragmentState {
                    shader: pipelines.shader.clone(),
                    shader_defs: defs,
                    targets: vec![Some(ColorTargetState {
                        format,
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    })],
                    ..default()
                }),
                ..default()
            })
        });
        commands.entity(entity).insert(VgDebugPipeline { id, depth });
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn pass(
    view: ViewQuery<(
        &ViewTarget,
        &ViewUniformOffset,
        &VgViewBindGroups,
        &VgDebugPipeline,
        Option<&ViewPrepassTextures>,
    )>,
    debug: Res<VgDebug>,
    pipelines: Res<DebugPipelines>,
    pipeline_cache: Res<PipelineCache>,
    view_uniforms: Res<ViewUniforms>,
    device: Res<RenderDevice>,
    settings: Option<Res<super::view::VirtualGeometrySettingsRender>>,
    mut ctx: RenderContext,
) {
    let (target, view_offset, groups, pipeline, prepass) = view.into_inner();
    let (Some(render_pipeline), Some(view_binding)) = (
        pipeline_cache.get_render_pipeline(pipeline.id),
        view_uniforms.uniforms.binding(),
    ) else {
        return;
    };
    let depth_view = prepass.and_then(|p| p.depth_only_view());
    if pipeline.depth && depth_view.is_none() {
        return;
    }

    // The residency view needs the cull pass's threshold to tell a cluster
    // at the detail the camera wants from a coarser stand-in.
    let threshold = settings.map_or(1.0, |s| s.lod_error_pixels);
    let params = device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("vg_debug_params"),
        contents: bytemuck::cast_slice(&[debug.mode, threshold.to_bits(), 0, 0]),
        usage: BufferUsages::UNIFORM,
    });
    let post = target.post_process_write();
    let layout = pipeline_cache.get_bind_group_layout(if pipeline.depth {
        &pipelines.layout_with_depth
    } else {
        &pipelines.layout
    });
    let view_group = match depth_view {
        Some(depth) if pipeline.depth => device.create_bind_group(
            "vg_debug",
            &layout,
            &BindGroupEntries::sequential((
                view_binding,
                post.source,
                params.as_entire_binding(),
                depth,
            )),
        ),
        _ => device.create_bind_group(
            "vg_debug",
            &layout,
            &BindGroupEntries::sequential((view_binding, post.source, params.as_entire_binding())),
        ),
    };

    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("vg_debug"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: post.destination,
            depth_slice: None,
            resolve_target: None,
            ops: Operations::default(),
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    // The whole target, not the camera viewport: the copy has to fill the
    // destination everywhere, and the shader works in target pixels.
    pass.set_render_pipeline(render_pipeline);
    pass.set_bind_group(0, &groups.shade, &[]);
    pass.set_bind_group(1, &view_group, &[view_offset.offset]);
    pass.draw(0..3, 0..1);
}

/// Where the pass sits in Bevy's `Core3d` schedule: last in the main pass,
/// before anti-aliasing and DLSS.
///
/// It ran after tonemapping at first, and the image shook. The visibility
/// buffer is rasterized with the camera's sub-pixel jitter, which TAA and DLSS
/// resolve for everything they see; an overlay drawn after them was the one
/// thing left jittering, and with DLSS it was also in the wrong place, since
/// the frame is still at render resolution before DLSS upscales it. Drawn here,
/// the colours go through the same resolve as the rest of the frame (and
/// through tonemapping, which shifts them a little).
pub(crate) fn add_pass(render_app: &mut SubApp) {
    use bevy::core_pipeline::core_3d::main_transparent_pass_3d;
    use bevy::core_pipeline::schedule::{Core3d, Core3dSystems};
    render_app.add_systems(
        Core3d,
        pass.after(main_transparent_pass_3d).in_set(Core3dSystems::MainPass),
    );
}
