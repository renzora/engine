use std::ops::Range;

use bevy::ecs::entity::EntityHash;
use bevy::math::FloatOrd;
use bevy::prelude::*;
use indexmap::IndexMap;
use bevy::render::camera::ExtractedCamera;
use bevy::render::mesh::allocator::MeshSlabs;
use bevy::render::render_phase::{
    BinnedPhaseItem, CachedRenderPipelinePhaseItem, DrawFunctionId, PhaseItem,
    PhaseItemBatchSetKey, PhaseItemExtraIndex, SortedPhaseItem, ViewBinnedRenderPhases,
    ViewSortedRenderPhases,
};
use bevy::render::render_resource::{
    CachedRenderPipelineId, Operations, RenderPassDepthStencilAttachment, RenderPassDescriptor,
    StoreOp,
};
use bevy::render::renderer::{RenderContext, ViewQuery};
use bevy::render::sync_world::MainEntity;
use bevy::render::view::{ExtractedView, ViewDepthTexture, ViewTarget};
use wgpu_types::ImageSubresourceRange;

use crate::view_uniforms::OutlineQueueStatus;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct OutlineBatchSetKey {
    pub pipeline: CachedRenderPipelineId,
    pub draw_function: DrawFunctionId,
    // Bevy 0.19: the mesh allocator's per-mesh slab IDs are bundled into
    // `MeshSlabs` (vertex + optional index, mirroring `Opaque3dBatchSetKey`),
    // replacing the old separate `vertex_slab`/`index_slab: SlabId` fields.
    pub slabs: MeshSlabs,
}

impl PhaseItemBatchSetKey for OutlineBatchSetKey {
    fn indexed(&self) -> bool {
        self.slabs.index_slab_id.is_some()
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

    // Bevy 0.19: `recalculate_sort_keys` is now a required method (no default).
    // The outline's sort key is the precomputed view distance; it does not
    // change per re-sort, so there is nothing to recompute here (the distance
    // is set once at queue time, mirroring how a custom phase with a fixed
    // distance behaves).
    fn recalculate_sort_keys(
        _items: &mut IndexMap<(Entity, MainEntity), Self, EntityHash>,
        _view: &ExtractedView,
    ) {
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

/// The outline pass, as a Bevy 0.19 render system (the old `OutlineNode:
/// ViewNode` render-graph node). Runs in the `Core3d` schedule after the main
/// passes; draws the stencil, opaque, and transparent outline sub-phases for
/// the current view. Mirrors bevy's own `main_opaque_pass_3d` system shape.
#[allow(clippy::type_complexity)]
pub(crate) fn outline_pass(
    world: &World,
    view: ViewQuery<(
        &'static ExtractedView,
        &'static ExtractedCamera,
        &'static Camera3d,
        &'static ViewTarget,
        &'static ViewDepthTexture,
        &'static OutlineQueueStatus,
    )>,
    stencil_phases: Res<ViewBinnedRenderPhases<StencilOutline>>,
    opaque_phases: Res<ViewBinnedRenderPhases<OpaqueOutline>>,
    transparent_phases: Res<ViewSortedRenderPhases<TransparentOutline>>,
    mut render_context: RenderContext,
) {
    let view_entity = view.entity();
    let (view, camera, camera_3d, target, depth, queue_status) = view.into_inner();

    let (Some(stencil_phase), Some(opaque_phase), Some(transparent_phase)) = (
        stencil_phases.get(&view.retained_view_entity),
        opaque_phases.get(&view.retained_view_entity),
        transparent_phases.get(&view.retained_view_entity),
    ) else {
        return;
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
            multiview_mask: None,
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
            multiview_mask: None,
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
            multiview_mask: None,
        };
        let mut tracked_pass = render_context.begin_tracked_render_pass(pass_descriptor);
        if let Some(viewport) = camera.viewport.as_ref() {
            tracked_pass.set_camera_viewport(viewport);
        }
        if let Err(err) = transparent_phase.render(&mut tracked_pass, world, view_entity) {
            error!("Error encountered while rendering the outline transparent phase {err:?}");
        }
    }
}
