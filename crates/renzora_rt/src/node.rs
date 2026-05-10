use bevy::core_pipeline::prepass::ViewPrepassTextures;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::render_graph::{NodeRunError, RenderGraphContext, ViewNode};
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderContext;
use bevy::render::view::{ViewTarget, ViewUniformOffset, ViewUniforms};

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

        // post_process_write swaps source/destination each frame to support
        // read-then-write of the same logical view target. Build the bind
        // group fresh every frame against the current source.
        let post_process = view_target.post_process_write();

        let bind_group = render_context.render_device().create_bind_group(
            "rt_ssgi_bind_group",
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
            label: Some("rt_ssgi_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: post_process.destination,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
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
