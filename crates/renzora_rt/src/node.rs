use bevy::core_pipeline::prepass::ViewPrepassTextures;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::render_graph::{NodeRunError, RenderGraphContext, ViewNode};
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderContext;
use bevy::render::view::{ViewTarget, ViewUniformOffset, ViewUniforms};
use std::sync::atomic::Ordering;

use crate::prepare::{RtPipeline, RtViewResources};
use crate::RtLighting;

#[derive(Default)]
pub struct RtNode;

impl ViewNode for RtNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static ViewUniformOffset,
        &'static ViewPrepassTextures,
        &'static RtViewResources,
        &'static RtLighting,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, view_offset, prepass, resources, settings): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        if !settings.enabled || settings.intensity <= 0.0 {
            return Ok(());
        }

        let pipeline = world.resource::<RtPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let view_uniforms = world.resource::<ViewUniforms>();

        let Some(render_pipeline) = pipeline_cache.get_render_pipeline(pipeline.pipeline_id) else {
            return Ok(());
        };
        let Some(view_binding) = view_uniforms.uniforms.binding() else {
            return Ok(());
        };
        let Some(depth_view) = prepass.depth_view() else { return Ok(()); };
        let Some(normal_view) = prepass.normal_view() else { return Ok(()); };

        // Frame-count parity decides which history view is "previous"
        // (read) and which is "next" (written this frame). We use the
        // most-recent value the prepare system stamped: it incremented
        // before writing the uniform, so frame_count - 1 matches what
        // the shader sees in `config.frame_count`.
        let frame = pipeline.frame_count.load(Ordering::Relaxed).wrapping_sub(1);
        let (read_history, write_history) = if frame % 2 == 0 {
            (&resources.history_a, &resources.history_b)
        } else {
            (&resources.history_b, &resources.history_a)
        };

        let post_process = view_target.post_process_write();

        let bind_group = render_context.render_device().create_bind_group(
            "rt_ssgi_bind_group",
            &pipeline_cache.get_bind_group_layout(&pipeline.layout),
            &BindGroupEntries::sequential((
                post_process.source,
                &pipeline.sampler,
                depth_view,
                normal_view,
                read_history,
                view_binding.clone(),
                resources.config_buffer.as_entire_binding(),
            )),
        );

        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("rt_ssgi_pass"),
            color_attachments: &[
                // Target 0: composite — scene + indirect → view target dest.
                Some(RenderPassColorAttachment {
                    view: post_process.destination,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations::default(),
                }),
                // Target 1: new history — indirect.rgb in RGB, linear depth in A.
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
