use std::ops::Range;

use bevy::ecs::query::QueryItem;
use bevy::math::FloatOrd;
use bevy::prelude::*;
use bevy::render::camera::ExtractedCamera;
use bevy::render::mesh::allocator::SlabId;
use bevy::render::render_graph::{NodeRunError, ViewNode};
use bevy::render::render_phase::{
    BinnedPhaseItem, CachedRenderPipelinePhaseItem, DrawFunctionId, PhaseItem,
    PhaseItemBatchSetKey, PhaseItemExtraIndex, SortedPhaseItem, ViewBinnedRenderPhases,
    ViewSortedRenderPhases,
};
use bevy::render::render_resource::{
    CachedRenderPipelineId, Operations, RenderPassDepthStencilAttachment, RenderPassDescriptor,
    StoreOp,
};
use bevy::render::sync_world::MainEntity;
use bevy::render::view::{ExtractedView, ViewDepthTexture, ViewTarget};
use bevy::render::{render_graph::RenderGraphContext, renderer::RenderContext};
use wgpu_types::ImageSubresourceRange;

use crate::view_uniforms::OutlineQueueStatus;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct OutlineBatchSetKey {
    pub pipeline: CachedRenderPipelineId,
    pub draw_function: DrawFunctionId,
    pub vertex_slab: SlabId,
    pub index_slab: Option<SlabId>,
}

impl PhaseItemBatchSetKey for OutlineBatchSetKey {
    fn indexed(&self) -> bool {
        self.index_slab.is_some()
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct OutlineBinKey {
    pub asset_id: AssetId<Mesh>,
    pub texture_id: Option<AssetId<Image>>,
}

pub(crate) struct StencilOutline {
    pub batch_set_key: OutlineBatchSetKey,
    pub entity: Entity,
    pub main_entity: MainEntity,
    pub batch_range: Range<u32>,
    pub extra_index: PhaseItemExtraIndex,
}

impl PhaseItem for StencilOutline {
    #[inline]
    fn entity(&self) -> Entity {
        self.entity
    }

    fn main_entity(&self) -> bevy::render::sync_world::MainEntity {
        self.main_entity
    }

    fn draw_function(&self) -> bevy::render::render_phase::DrawFunctionId {
        self.batch_set_key.draw_function
    }

    fn batch_range(&self) -> &std::ops::Range<u32> {
        &self.batch_range
    }

    fn batch_range_mut(&mut self) -> &mut std::ops::Range<u32> {
        &mut self.batch_range
    }

    fn extra_index(&self) -> bevy::render::render_phase::PhaseItemExtraIndex {
        self.extra_index.clone()
    }

    fn batch_range_and_extra_index_mut(
        &mut self,
    ) -> (
        &mut Range<u32>,
        &mut bevy::render::render_phase::PhaseItemExtraIndex,
    ) {
        (&mut self.batch_range, &mut self.extra_index)
    }
}

impl BinnedPhaseItem for StencilOutline {
    type BatchSetKey = OutlineBatchSetKey;
    type BinKey = OutlineBinKey;

    fn new(
        batch_set_key: Self::BatchSetKey,
        _bin_key: Self::BinKey,
        representative_entity: (Entity, MainEntity),
        batch_range: Range<u32>,
        extra_index: PhaseItemExtraIndex,
    ) -> Self {
        Self {
            batch_set_key,
            entity: representative_entity.0,
            main_entity: representative_entity.1,
            batch_range,
            extra_index,
        }
    }
}

impl CachedRenderPipelinePhaseItem for StencilOutline {
    #[inline]
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.batch_set_key.pipeline
    }
}

pub(crate) struct OpaqueOutline {
    pub batch_set_key: OutlineBatchSetKey,
    pub entity: Entity,
    pub main_entity: MainEntity,
    pub batch_range: Range<u32>,
    pub extra_index: PhaseItemExtraIndex,
}

impl PhaseItem for OpaqueOutline {
    #[inline]
    fn entity(&self) -> Entity {
        self.entity
    }

    fn main_entity(&self) -> bevy::render::sync_world::MainEntity {
        self.main_entity
    }

    fn draw_function(&self) -> bevy::render::render_phase::DrawFunctionId {
        self.batch_set_key.draw_function
    }

    fn batch_range(&self) -> &Range<u32> {
        &self.batch_range
    }

    fn batch_range_mut(&mut self) -> &mut Range<u32> {
        &mut self.batch_range
    }

    fn extra_index(&self) -> bevy::render::render_phase::PhaseItemExtraIndex {
        self.extra_index.clone()
    }

    fn batch_range_and_extra_index_mut(
        &mut self,
    ) -> (
        &mut Range<u32>,
        &mut bevy::render::render_phase::PhaseItemExtraIndex,
    ) {
        (&mut self.batch_range, &mut self.extra_index)
    }
}

impl BinnedPhaseItem for OpaqueOutline {
    type BatchSetKey = OutlineBatchSetKey;
    type BinKey = OutlineBinKey;

    fn new(
        batch_set_key: Self::BatchSetKey,
        _bin_key: Self::BinKey,
        representative_entity: (Entity, MainEntity),
        batch_range: Range<u32>,
        extra_index: PhaseItemExtraIndex,
    ) -> Self {
        OpaqueOutline {
            batch_set_key,
            entity: representative_entity.0,
            main_entity: representative_entity.1,
            batch_range,
            extra_index,
        }
    }
}

impl CachedRenderPipelinePhaseItem for OpaqueOutline {
    #[inline]
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.batch_set_key.pipeline
    }
}

pub(crate) struct TransparentOutline {
    pub distance: f32,
    pub pipeline: CachedRenderPipelineId,
    pub entity: Entity,
    pub main_entity: MainEntity,
    pub draw_function: DrawFunctionId,
    pub batch_range: Range<u32>,
    pub extra_index: PhaseItemExtraIndex,
    pub indexed: bool,
}

impl PhaseItem for TransparentOutline {
    #[inline]
    fn entity(&self) -> Entity {
        self.entity
    }

    fn main_entity(&self) -> bevy::render::sync_world::MainEntity {
        self.main_entity
    }

    fn draw_function(&self) -> bevy::render::render_phase::DrawFunctionId {
        self.draw_function
    }

    fn batch_range(&self) -> &Range<u32> {
        &self.batch_range
    }

    fn batch_range_mut(&mut self) -> &mut Range<u32> {
        &mut self.batch_range
    }

    fn extra_index(&self) -> PhaseItemExtraIndex {
        self.extra_index.clone()
    }

    fn batch_range_and_extra_index_mut(&mut self) -> (&mut Range<u32>, &mut PhaseItemExtraIndex) {
        (&mut self.batch_range, &mut self.extra_index)
    }
}

impl SortedPhaseItem for TransparentOutline {
    type SortKey = FloatOrd;

    fn sort_key(&self) -> Self::SortKey {
        FloatOrd(self.distance)
    }

    fn indexed(&self) -> bool {
        self.indexed
    }
}

impl CachedRenderPipelinePhaseItem for TransparentOutline {
    #[inline]
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.pipeline
    }
}

pub(crate) struct OutlineNode;

impl FromWorld for OutlineNode {
    fn from_world(_world: &mut World) -> Self {
        Self
    }
}

impl ViewNode for OutlineNode {
    type ViewQuery = (
        &'static ExtractedView,
        &'static ExtractedCamera,
        &'static Camera3d,
        &'static ViewTarget,
        &'static ViewDepthTexture,
        &'static OutlineQueueStatus,
    );

    fn run<'w>(
        &self,
        graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (view, camera, camera_3d, target, depth, queue_status): QueryItem<'w, 'w, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let view_entity = graph.view_entity();
        let (Some(stencil_phase), Some(opaque_phase), Some(transparent_phase)) = (
            world
                .get_resource::<ViewBinnedRenderPhases<StencilOutline>>()
                .and_then(|ps| ps.get(&view.retained_view_entity)),
            world
                .get_resource::<ViewBinnedRenderPhases<OpaqueOutline>>()
                .and_then(|ps| ps.get(&view.retained_view_entity)),
            world
                .get_resource::<ViewSortedRenderPhases<TransparentOutline>>()
                .and_then(|ps| ps.get(&view.retained_view_entity)),
        ) else {
            return Ok(());
        };

        // If drawing anything, run stencil pass to clear the depth buffer
        if queue_status.has_volume {
            render_context
                .command_encoder()
                .clear_texture(&depth.texture, &ImageSubresourceRange::default());

            let pass_descriptor = RenderPassDescriptor {
                label: Some("outline_stencil_pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: depth.view(),
                    depth_ops: Some(Operations {
                        load: camera_3d.depth_load_op.clone().into(),
                        store: StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            };
            let mut tracked_pass = render_context.begin_tracked_render_pass(pass_descriptor);
            if let Some(viewport) = camera.viewport.as_ref() {
                tracked_pass.set_camera_viewport(viewport);
            }
            if let Err(err) = stencil_phase.render(&mut tracked_pass, world, view_entity) {
                error!("Error encountered while rendering the outline stencil phase {err:?}");
            }
        }

        if !opaque_phase.is_empty() {
            let pass_descriptor = RenderPassDescriptor {
                label: Some("outline_opaque_pass"),
                color_attachments: &[Some(target.get_color_attachment())],
                depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
                timestamp_writes: None,
                occlusion_query_set: None,
            };
            let mut tracked_pass = render_context.begin_tracked_render_pass(pass_descriptor);
            if let Some(viewport) = camera.viewport.as_ref() {
                tracked_pass.set_camera_viewport(viewport);
            }
            if let Err(err) = opaque_phase.render(&mut tracked_pass, world, view_entity) {
                error!("Error encountered while rendering the outline opaque phase {err:?}");
            }
        }

        if !transparent_phase.items.is_empty() {
            let pass_descriptor = RenderPassDescriptor {
                label: Some("outline_transparent_pass"),
                color_attachments: &[Some(target.get_color_attachment())],
                depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
                timestamp_writes: None,
                occlusion_query_set: None,
            };
            let mut tracked_pass = render_context.begin_tracked_render_pass(pass_descriptor);
            if let Some(viewport) = camera.viewport.as_ref() {
                tracked_pass.set_camera_viewport(viewport);
            }
            if let Err(err) = transparent_phase.render(&mut tracked_pass, world, view_entity) {
                error!("Error encountered while rendering the outline opaque phase {err:?}");
            }
        }

        Ok(())
    }
}
