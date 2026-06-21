use bevy::{
    core_pipeline::blit::{BlitPipeline, BlitPipelineKey},
    ecs::{
        component::Component,
        entity::Entity,
        system::{Commands, Query, Res, ResMut},
        world::World,
    },
    render::{
        render_resource::{
            BindGroupEntries, CachedRenderPipelineId, LoadOp, Operations, PipelineCache,
            RenderPassColorAttachment, RenderPassDescriptor, SpecializedRenderPipelines, StoreOp,
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        view::{Msaa, ViewTarget},
    },
};

#[derive(Component)]
pub(crate) struct MsaaExtraWritebackPipeline(CachedRenderPipelineId);

/// MSAA extra-writeback pass, as a Bevy 0.19 render system (was
/// `MsaaExtraWritebackNode: ViewNode`). Only runs for views that carry a
/// `MsaaExtraWritebackPipeline` (i.e. MSAA enabled) — `ViewQuery` silently
/// skips non-matching views, exactly as the old ViewNode query filter did.
pub(crate) fn msaa_extra_writeback(
    world: &World,
    view: ViewQuery<(&'static ViewTarget, &'static MsaaExtraWritebackPipeline)>,
    mut render_context: RenderContext,
) {
    let (target, blit_pipeline_id) = view.into_inner();

    let blit_pipeline = world.resource::<BlitPipeline>();
    let pipeline_cache = world.resource::<PipelineCache>();
    let Some(pipeline) = pipeline_cache.get_render_pipeline(blit_pipeline_id.0) else {
        return;
    };

    let post_process = target.post_process_write();

    let pass_descriptor = RenderPassDescriptor {
        label: Some("msaa_extra_writeback"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: target.sampled_main_texture_view().unwrap(),
            resolve_target: Some(post_process.destination),
            ops: Operations {
                load: LoadOp::Clear(wgpu_types::Color::BLACK),
                store: StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    };

    // Create bind group layout from the blit pipeline's descriptor
    let render_device = world.resource::<RenderDevice>();
    let layout = render_device
        .create_bind_group_layout("blit_bind_group_layout", &blit_pipeline.layout.entries);
    let bind_group = render_context.render_device().create_bind_group(
        None,
        &layout,
        &BindGroupEntries::sequential((post_process.source, &blit_pipeline.sampler)),
    );

    let mut render_pass = render_context
        .command_encoder()
        .begin_render_pass(&pass_descriptor);

    render_pass.set_pipeline(pipeline);
    render_pass.set_bind_group(0, &bind_group, &[]);
    render_pass.draw(0..3, 0..1);
}

pub(crate) fn prepare_msaa_extra_writeback_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    mut pipelines: ResMut<SpecializedRenderPipelines<BlitPipeline>>,
    blit_pipeline: Res<BlitPipeline>,
    view_targets: Query<(Entity, &ViewTarget, &Msaa)>,
) {
    for (entity, view_target, msaa) in view_targets.iter() {
        if *msaa != Msaa::Off {
            // Bevy 0.19: `texture_format` → `target_format`, and a new
            // `source_space` field (None = no color-space conversion, a plain blit).
            let key = BlitPipelineKey {
                target_format: view_target.main_texture_format(),
                samples: msaa.samples(),
                blend_state: None,
                source_space: None,
            };

            let pipeline = pipelines.specialize(&pipeline_cache, &blit_pipeline, key);
            commands
                .entity(entity)
                .insert(MsaaExtraWritebackPipeline(pipeline));
        } else {
            commands
                .entity(entity)
                .remove::<MsaaExtraWritebackPipeline>();
        }
    }
}
