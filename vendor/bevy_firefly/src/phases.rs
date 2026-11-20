//! Module containing custom render phases.

use std::ops::Range;

use bevy::ecs::entity::EntityHash;
use bevy::math::FloatOrd;
use bevy::prelude::*;
use bevy::render::render_phase::{
    BinnedPhaseItem, CachedRenderPipelinePhaseItem, DrawFunctionId, PhaseItem,
    PhaseItemBatchSetKey, PhaseItemExtraIndex, SortedPhaseItem,
};
use bevy::render::render_resource::CachedRenderPipelineId;
use bevy::render::sync_world::MainEntity;
use bevy::render::view::ExtractedView;
use indexmap::IndexMap;

/// Binned Render Phase that uses lights to render the lightmap texture.
pub struct LightmapPhase {
    batch_set_key: LightBatchSetKey,
    pub entity: (Entity, MainEntity),
    pub batch_range: Range<u32>,
    pub extra_index: PhaseItemExtraIndex,
}

/// Sorted Render Phase that uses sprites to render the stencil and normal textures.
pub struct SpritePhase {
    pub sort_key: FloatOrd,
    pub entity: (Entity, MainEntity),
    pub pipeline: CachedRenderPipelineId,
    pub draw_function: DrawFunctionId,
    pub batch_range: Range<u32>,
    pub extra_index: PhaseItemExtraIndex,
    pub extracted_index: usize,
    /// Whether the mesh in question is indexed (uses an index buffer in
    /// addition to its vertex buffer).
    pub indexed: bool,
}

// For more information about writing a phase item, please look at the custom_phase_item example
impl PhaseItem for LightmapPhase {
    #[inline]
    fn entity(&self) -> Entity {
        self.entity.0
    }

    #[inline]
    fn main_entity(&self) -> MainEntity {
        self.entity.1
    }

    #[inline]
    fn draw_function(&self) -> DrawFunctionId {
        self.batch_set_key.draw_function
    }

    #[inline]
    fn batch_range(&self) -> &Range<u32> {
        &self.batch_range
    }

    #[inline]
    fn batch_range_mut(&mut self) -> &mut Range<u32> {
        &mut self.batch_range
    }

    #[inline]
    fn extra_index(&self) -> PhaseItemExtraIndex {
        self.extra_index.clone()
    }

    #[inline]
    fn batch_range_and_extra_index_mut(&mut self) -> (&mut Range<u32>, &mut PhaseItemExtraIndex) {
        (&mut self.batch_range, &mut self.extra_index)
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LightBatchSetKey {
    pub pipeline: CachedRenderPipelineId,
    pub draw_function: DrawFunctionId,
}

impl PhaseItemBatchSetKey for LightBatchSetKey {
    fn indexed(&self) -> bool {
        false
    }
}

impl BinnedPhaseItem for LightmapPhase {
    type BinKey = ();

    type BatchSetKey = LightBatchSetKey;

    fn new(
        batch_set_key: Self::BatchSetKey,
        _bin_key: Self::BinKey,
        representative_entity: (Entity, MainEntity),
        batch_range: Range<u32>,
        extra_index: PhaseItemExtraIndex,
    ) -> Self {
        Self {
            batch_set_key,
            entity: representative_entity,
            batch_range,
            extra_index,
        }
    }
}

impl CachedRenderPipelinePhaseItem for LightmapPhase {
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.batch_set_key.pipeline
    }
}

impl PhaseItem for SpritePhase {
    #[inline]
    fn entity(&self) -> Entity {
        self.entity.0
    }

    #[inline]
    fn main_entity(&self) -> MainEntity {
        self.entity.1
    }

    #[inline]
    fn draw_function(&self) -> DrawFunctionId {
        self.draw_function
    }

    #[inline]
    fn batch_range(&self) -> &Range<u32> {
        &self.batch_range
    }

    #[inline]
    fn batch_range_mut(&mut self) -> &mut Range<u32> {
        &mut self.batch_range
    }

    #[inline]
    fn extra_index(&self) -> PhaseItemExtraIndex {
        self.extra_index.clone()
    }

    #[inline]
    fn batch_range_and_extra_index_mut(&mut self) -> (&mut Range<u32>, &mut PhaseItemExtraIndex) {
        (&mut self.batch_range, &mut self.extra_index)
    }
}

impl SortedPhaseItem for SpritePhase {
    type SortKey = FloatOrd;

    #[inline]
    fn sort_key(&self) -> Self::SortKey {
        self.sort_key
    }

    #[inline]
    fn sort(items: &mut IndexMap<(Entity, MainEntity), SpritePhase, EntityHash>) {
        items.sort_by_key(|_, phase_item| phase_item.sort_key);
    }

    fn recalculate_sort_keys(
        _: &mut IndexMap<(Entity, MainEntity), Self, EntityHash>,
        _: &ExtractedView,
    ) {
        // Sort keys are precalculated for 2D phase items.
    }

    #[inline]
    fn indexed(&self) -> bool {
        self.indexed
    }
}

impl CachedRenderPipelinePhaseItem for SpritePhase {
    #[inline]
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.pipeline
    }
}
