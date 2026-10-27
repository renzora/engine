//! The passes, in the order a frame runs them.
//!
//! [`raster`] runs first in Bevy's prepass, before any ordinary mesh: it culls,
//! rasterizes into the visibility buffer and the view's depth, then writes the
//! material-depth buffer. Being first, it is what clears the view's depth;
//! ordinary meshes drawn afterwards load and test against it, so the two kinds
//! of geometry occlude each other with no extra work.
//!
//! [`prepass`] then writes the prepass targets (normals, motion vectors, the
//! deferred G-buffer) for converted pixels, and [`main_opaque`] shades the
//! forward ones into the view target, ahead of Bevy's own opaque pass. Bevy's
//! deferred lighting, SSAO, SSR, TAA, DLSS and Solari all read those targets
//! afterwards and never know which pixels came from where.

use bevy::camera::{MainPassResolutionOverride, Viewport};
use bevy::core_pipeline::prepass::{MotionVectorPrepass, PreviousViewUniformOffset, ViewPrepassTextures};
use bevy::pbr::{MeshViewBindGroup, PrepassViewBindGroup};
use bevy::prelude::*;
use bevy::render::camera::ExtractedCamera;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, ViewQuery};
use bevy::render::view::{ViewDepthStencilTexture, ViewTarget, ViewUniformOffset};

use super::masked::MaskedDraws;
use super::materials::VgViewMaterials;
use super::pipelines::VgPipelines;
use super::scene::VgSkinBindGroup;
use super::view::{VgView, VgViewBindGroups, BUCKET_STRIDE};

#[allow(clippy::type_complexity)]
pub(crate) fn raster(
    view: ViewQuery<(
        &ExtractedCamera,
        &ViewDepthStencilTexture,
        &ViewUniformOffset,
        &VgView,
        &VgViewBindGroups,
        Option<&MainPassResolutionOverride>,
    )>,
    pipelines: Res<VgPipelines>,
    masked: Res<MaskedDraws>,
    skin: Res<VgSkinBindGroup>,
    pipeline_cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (camera, depth, view_offset, vg, groups, resolution_override) = view.into_inner();
    let (Some(cull), Some(single), Some(double), Some(material_depth)) = (
        pipeline_cache.get_compute_pipeline(pipelines.cull),
        pipeline_cache.get_render_pipeline(pipelines.raster_single),
        pipeline_cache.get_render_pipeline(pipelines.raster_double),
        pipeline_cache.get_render_pipeline(pipelines.material_depth),
    ) else {
        return;
    };
    let viewport = Viewport::from_viewport_and_override(camera.viewport.as_ref(), resolution_override);

    ctx.command_encoder().push_debug_group("virtual_geometry_raster");
    // Deform the skinned instances first. It runs once per view, though the
    // result does not depend on the view: every view writes the same values,
    // and doing it here is what guarantees it precedes whichever view's
    // raster the GPU happens to reach first.
    if let (Some((bind_group, vertices)), Some(pipeline)) =
        (skin.0.as_ref(), pipeline_cache.get_compute_pipeline(pipelines.skin))
    {
        let groups = vertices.div_ceil(64);
        let x = groups.min(65535);
        let mut pass = ctx.command_encoder().begin_compute_pass(&ComputePassDescriptor {
            label: Some("vg_skin"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.dispatch_workgroups(x, groups.div_ceil(x.max(1)), 1);
    }
    {
        let mut pass = ctx.command_encoder().begin_compute_pass(&ComputePassDescriptor {
            label: Some("vg_cull"),
            timestamp_writes: None,
        });
        pass.set_pipeline(cull);
        pass.set_bind_group(0, &groups.cull, &[view_offset.offset]);
        pass.dispatch_workgroups(vg.dispatch.0, vg.dispatch.1, 1);
    }
    {
        let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("vg_raster"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &vg.visibility.default_view,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(LinearRgba::NONE.into()),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if let Some(viewport) = viewport.as_ref() {
            pass.set_camera_viewport(viewport);
        }
        pass.set_bind_group(0, &groups.raster_view, &[view_offset.offset]);
        pass.set_bind_group(1, &groups.raster_scene, &[]);
        pass.set_render_pipeline(single);
        pass.draw_indirect(&vg.draws, 0);
        pass.set_render_pipeline(double);
        pass.draw_indirect(&vg.draws, 16);

        // Alpha-masked clusters, one draw per bucket, each with its
        // materials' bind group and its first slot.
        pass.set_bind_group(1, &groups.masked_scene, &[]);
        for (i, draw) in masked.0.iter().enumerate().take(vg.masked_count as usize) {
            let Some(draw) = draw else {
                continue;
            };
            let Some(pipeline) = pipeline_cache.get_render_pipeline(draw.pipeline) else {
                continue;
            };
            pass.set_render_pipeline(pipeline);
            pass.set_bind_group(2, &draw.material, &[]);
            pass.set_bind_group(3, &groups.buckets, &[i as u32 * BUCKET_STRIDE]);
            pass.draw_indirect(&vg.masked_draws, i as u64 * 16);
        }
    }
    {
        let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("vg_material_depth"),
            color_attachments: &[],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &vg.material_depth.default_view,
                depth_ops: Some(Operations {
                    load: LoadOp::Clear(0.0),
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if let Some(viewport) = viewport.as_ref() {
            pass.set_camera_viewport(viewport);
        }
        pass.set_render_pipeline(material_depth);
        pass.set_bind_group(0, &groups.material_depth, &[]);
        pass.draw(0..3, 0..1);
    }
    ctx.command_encoder().pop_debug_group();
}

fn material_depth_attachment(vg: &VgView) -> RenderPassDepthStencilAttachment<'_> {
    RenderPassDepthStencilAttachment {
        view: &vg.material_depth.default_view,
        depth_ops: Some(Operations {
            load: LoadOp::Load,
            store: StoreOp::Store,
        }),
        stencil_ops: None,
    }
}

/// One fullscreen triangle per material, at its material depth.
fn draw_materials<'w>(
    pass: &mut bevy::render::render_phase::TrackedRenderPass<'w>,
    materials: &'w [(u16, CachedRenderPipelineId, BindGroup)],
    pipeline_cache: &'w PipelineCache,
) {
    for (id, pipeline, bind_group) in materials {
        let Some(pipeline) = pipeline_cache.get_render_pipeline(*pipeline) else {
            continue;
        };
        let first = *id as u32 * 3;
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(3, bind_group, &[]);
        pass.draw(first..first + 3, 0..1);
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn prepass(
    view: ViewQuery<(
        &ExtractedCamera,
        &ViewPrepassTextures,
        &ViewUniformOffset,
        &PreviousViewUniformOffset,
        Has<MotionVectorPrepass>,
        &VgView,
        &VgViewBindGroups,
        &VgViewMaterials,
        Option<&MainPassResolutionOverride>,
    )>,
    prepass_view: Res<PrepassViewBindGroup>,
    pipeline_cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (camera, textures, view_offset, previous_offset, motion_vectors, vg, groups, materials, resolution_override) =
        view.into_inner();
    let viewport = Viewport::from_viewport_and_override(camera.viewport.as_ref(), resolution_override);

    for (deferred, list) in [(false, &materials.prepass), (true, &materials.deferred)] {
        if list.is_empty() {
            continue;
        }
        let (view_group, offsets) = if motion_vectors {
            (
                prepass_view.motion_vectors.as_ref(),
                vec![view_offset.offset, previous_offset.offset],
            )
        } else {
            (prepass_view.no_motion_vectors.as_ref(), vec![view_offset.offset])
        };
        let Some(view_group) = view_group else {
            continue;
        };
        // The same four slots Bevy's prepass pipelines declare, with the
        // G-buffer ones left out of a plain prepass.
        let color_attachments = [
            textures.normal.as_ref().map(|t| t.get_attachment()),
            textures.motion_vectors.as_ref().map(|t| t.get_attachment()),
            textures
                .deferred
                .as_ref()
                .filter(|_| deferred)
                .map(|t| t.get_attachment()),
            textures
                .deferred_lighting_pass_id
                .as_ref()
                .filter(|_| deferred)
                .map(|t| t.get_attachment()),
        ];
        let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some(if deferred { "vg_deferred_gbuffer" } else { "vg_prepass" }),
            color_attachments: &color_attachments,
            depth_stencil_attachment: Some(material_depth_attachment(vg)),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if let Some(viewport) = viewport.as_ref() {
            pass.set_camera_viewport(viewport);
        }
        pass.set_bind_group(0, view_group, &offsets);
        pass.set_bind_group(1, &prepass_view.empty_bind_group, &[]);
        pass.set_bind_group(2, &groups.shade, &[]);
        draw_materials(&mut pass, list, &pipeline_cache);
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn main_opaque(
    view: ViewQuery<(
        &ExtractedCamera,
        &ViewTarget,
        &MeshViewBindGroup,
        &VgView,
        &VgViewBindGroups,
        &VgViewMaterials,
        Option<&MainPassResolutionOverride>,
    )>,
    pipeline_cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (camera, target, mesh_view, vg, groups, materials, resolution_override) = view.into_inner();
    if materials.forward.is_empty() {
        return;
    }
    let viewport = Viewport::from_viewport_and_override(camera.viewport.as_ref(), resolution_override);
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("vg_main_opaque"),
        color_attachments: &[Some(target.get_color_attachment())],
        depth_stencil_attachment: Some(material_depth_attachment(vg)),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    if let Some(viewport) = viewport.as_ref() {
        pass.set_camera_viewport(viewport);
    }
    pass.set_bind_group(0, &mesh_view.main, &mesh_view.main_offsets);
    pass.set_bind_group(1, &mesh_view.binding_array, &[]);
    pass.set_bind_group(2, &groups.shade, &[]);
    draw_materials(&mut pass, &materials.forward, &pipeline_cache);
}
