use bevy::ecs::query::QueryItem;
use bevy::render::render_phase::{
    CachedRenderPipelinePhaseItem, DrawFunctionId, PhaseItem, ViewSortedRenderPhases,
};
use bevy::render::view::{ExtractedView, ViewDepthTexture};
use bevy::{
    math::FloatOrd,
    prelude::*,
    render::{
        camera::ExtractedCamera,
        render_graph::{NodeRunError, RenderGraphContext, ViewNode},
        render_phase::{PhaseItemExtraIndex, SortedPhaseItem},
        render_resource::CachedRenderPipelineId,
        renderer::RenderContext,
        sync_world::MainEntity,
        view::ViewTarget,
    },
};
use itertools::*;
use std::ops::Range;

use crate::OutlineViewUniform;

use super::compose_output::{ComposeOutputPass, ComposeOutputView};
use super::flood_init::FloodInitPass;
use super::jump_flood::JumpFloodPass;
use super::FloodTextures;

#[derive(Debug)]
pub struct FloodOutline {
    pub distance: f32,
    pub entity: Entity,
    pub main_entity: MainEntity,
    pub pipeline: CachedRenderPipelineId,
    pub draw_function: DrawFunctionId,
    pub batch_range: Range<u32>,
    pub extra_index: PhaseItemExtraIndex,
    pub indexed: bool,
    pub volume_offset: f32,
    pub volume_colour: Vec4,
    pub screen_space_bounds: URect,
}

impl PhaseItem for FloodOutline {
    #[inline]
    fn entity(&self) -> Entity {
        self.entity
    }

    #[inline]
    fn main_entity(&self) -> MainEntity {
        self.main_entity
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

impl CachedRenderPipelinePhaseItem for FloodOutline {
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.pipeline
    }
}

impl SortedPhaseItem for FloodOutline {
    type SortKey = FloatOrd;

    fn sort_key(&self) -> Self::SortKey {
        FloatOrd(self.distance)
    }

    fn indexed(&self) -> bool {
        self.indexed
    }
}

pub(crate) struct FloodNode;

impl FromWorld for FloodNode {
    fn from_world(_world: &mut World) -> Self {
        Self
    }
}

impl ViewNode for FloodNode {
    type ViewQuery = (
        &'static ExtractedView,
        &'static ExtractedCamera,
        &'static ViewTarget,
        &'static ViewDepthTexture,
        &'static OutlineViewUniform,
        &'static FloodTextures,
        &'static ComposeOutputView,
    );

    fn run<'w>(
        &self,
        graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (view, camera, target, depth, view_uniform, flood_textures, compose_output_view): QueryItem<
            'w,
            Self::ViewQuery,
        >,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let view_entity = graph.view_entity();
        let Some(flood_phase) = world
            .get_resource::<ViewSortedRenderPhases<FloodOutline>>()
            .and_then(|ps| ps.get(&view.retained_view_entity))
        else {
            return Ok(());
        };

        let mut flood_textures = flood_textures.clone();

        let mut flood_init_pass = FloodInitPass::new(world, view_entity, flood_phase, camera);
        let Some(mut jump_flood_pass) = JumpFloodPass::new(world) else {
            return Ok(());
        };
        let Some(compose_output_pass) =
            ComposeOutputPass::new(world, compose_output_view, target, depth)
        else {
            return Ok(());
        };

        for ((_, volume_offset, _), group) in &flood_phase
            .items
            .iter()
            .enumerate()
            .chunk_by(|(_, item)| (item.distance, item.volume_offset, item.volume_colour))
        {
            let mut group_iter = group.into_iter();
            let Some((first_index, first_item)) = group_iter.next() else {
                continue;
            };

            // Sum item range and screen-space bounds
            let mut last_index = first_index;
            let mut screen_space_bounds = first_item.screen_space_bounds;
            for (index, item) in group_iter {
                last_index = index;
                screen_space_bounds = screen_space_bounds.union(item.screen_space_bounds);
            }

            flood_init_pass.execute(
                render_context,
                first_index..last_index + 1,
                flood_textures.output(),
            );
            flood_textures.flip();

            let scaled_offset = view_uniform.scale_physical_from_logical * volume_offset;
            let passes = if scaled_offset > 0.0 {
                (scaled_offset.ceil() as u32 / 2 + 1)
                    .next_power_of_two()
                    .trailing_zeros()
                    + 1
            } else {
                0
            };

            for size in (0..passes).rev() {
                jump_flood_pass.execute(
                    render_context,
                    flood_textures.input(),
                    flood_textures.output(),
                    size,
                    &screen_space_bounds,
                );
                flood_textures.flip();
            }

            compose_output_pass.execute(
                render_context,
                view_entity,
                first_item.entity,
                flood_textures.input(),
                &screen_space_bounds,
            );
        }

        Ok(())
    }
}
