use std::ops::Range;

use bevy::prelude::*;
use bevy::render::camera::{ExtractedCamera, Viewport};
use bevy::render::mesh::allocator::MeshAllocator;
use bevy::render::render_phase::{DrawFunctions, PhaseItemExtraIndex, ViewSortedRenderPhases};
use bevy::render::sync_world::MainEntity;
use bevy::render::view::{ExtractedView, RenderLayers};
use bevy::render::{
    render_phase::SortedRenderPhase,
    render_resource::{
        LoadOp, Operations, RenderPassColorAttachment, RenderPassDescriptor, StoreOp,
    },
    renderer::RenderContext,
    texture::CachedTexture,
};
use wgpu_types::ImageSubresourceRange;

use crate::queue::{OutlineCache, OutlineCacheEntry, OutlineRangefinder};
use crate::uniforms::ExtractedOutline;
use crate::view_uniforms::OutlineQueueStatus;

use super::bounds::FloodMeshBounds;
use super::node::FloodOutline;
use super::{DrawMode, DrawOutline, OutlineViewUniform};

pub(crate) fn prepare_flood_phases(
    query: Query<&ExtractedView, With<OutlineViewUniform>>,
    mut flood_phases: ResMut<ViewSortedRenderPhases<FloodOutline>>,
) {
    for view in query.iter() {
        flood_phases.insert_or_clear(view.retained_view_entity);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn queue_flood_meshes(
    flood_draw_functions: Res<DrawFunctions<FloodOutline>>,
    mesh_allocator: Res<MeshAllocator>,
    outline_cache: Res<OutlineCache>,
    material_meshes: Query<(Entity, &MainEntity, &ExtractedOutline, &FloodMeshBounds)>,
    mut flood_phases: ResMut<ViewSortedRenderPhases<FloodOutline>>,
    mut views: Query<(
        &ExtractedView,
        &ExtractedCamera,
        Option<&RenderLayers>,
        &OutlineViewUniform,
        &mut OutlineQueueStatus,
    )>,
) {
    let draw_flood = flood_draw_functions.read().get_id::<DrawOutline>().unwrap();

    for (view, camera, view_mask, view_uniform, mut queue_status) in views.iter_mut() {
        let view_mask = view_mask.cloned().unwrap_or_default();

        let Some(flood_phase) = flood_phases.get_mut(&view.retained_view_entity) else {
            continue;
        };

        // Get the camera's physical target size for correct screen bounds calculation
        let fallback_viewport = Viewport {
            physical_size: camera.physical_target_size.unwrap_or_default(),
            ..default()
        };
        let viewport = camera.viewport.as_ref().unwrap_or(&fallback_viewport);

        let clip_from_world = view.clip_from_world.unwrap_or_else(|| {
            view.clip_from_view * view.world_from_view.compute_matrix().inverse()
        });

        let rangefinder = OutlineRangefinder::new(view);

        let outline_view_cache = outline_cache
            .view_map
            .get(&view.retained_view_entity)
            .unwrap();

        for (entity, main_entity, outline, mesh_bounds) in material_meshes.iter() {
            if !outline.volume {
                continue;
            }

            if !view_mask.intersects(&outline.layers) {
                continue;
            }

            if outline.draw_mode != DrawMode::JumpFlood {
                continue;
            }

            // Calculate screen-space bounds of outline
            let border = (view_uniform.scale_physical_from_logical
                * outline.instance_data.volume_offset)
                .ceil() as u32;
            let Some(screen_space_bounds) =
                mesh_bounds.calculate_screen_space_bounds(&clip_from_world, viewport, border)
            else {
                continue;
            };

            let (_vertex_slab, index_slab) = mesh_allocator.mesh_slabs(&outline.mesh_id);

            let Some(OutlineCacheEntry {
                changed_tick: _,
                stencil_pipeline_id: _,
                volume_pipeline_id,
            }) = outline_view_cache.entity_map.get(main_entity)
            else {
                continue;
            };

            queue_status.has_volume = true;

            flood_phase.add(FloodOutline {
                distance: rangefinder.distance_of(outline),
                entity,
                main_entity: *main_entity,
                pipeline: *volume_pipeline_id,
                draw_function: draw_flood,
                batch_range: 0..0,
                extra_index: PhaseItemExtraIndex::None,
                indexed: index_slab.is_some(),
                volume_offset: outline.instance_data.volume_offset,
                volume_colour: outline.instance_data.volume_colour,
                screen_space_bounds,
            });
        }
    }
}

pub(crate) struct FloodInitPass<'w> {
    world: &'w World,
    view_entity: Entity,
    flood_phase: &'w SortedRenderPhase<FloodOutline>,
    camera: &'w ExtractedCamera,
}

impl<'w> FloodInitPass<'w> {
    pub fn new(
        world: &'w World,
        view_entity: Entity,
        flood_phase: &'w SortedRenderPhase<FloodOutline>,
        camera: &'w ExtractedCamera,
    ) -> Self {
        Self {
            world,
            view_entity,
            flood_phase,
            camera,
        }
    }

    pub fn execute(
        &mut self,
        render_context: &mut RenderContext<'_>,
        range: Range<usize>,
        output: &CachedTexture,
    ) {
        render_context
            .command_encoder()
            .clear_texture(&output.texture, &ImageSubresourceRange::default());

        let color_attachment = RenderPassColorAttachment {
            view: &output.default_view,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(wgpu_types::Color {
                    r: -1.0,
                    g: -1.0,
                    b: -1.0,
                    a: 0.0,
                }),
                store: StoreOp::Store,
            },
        };

        let mut init_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("outline_flood_init"),
            color_attachments: &[Some(color_attachment)],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        if let Some(viewport) = self.camera.viewport.as_ref() {
            init_pass.set_camera_viewport(viewport);
        }

        if let Err(err) =
            self.flood_phase
                .render_range(&mut init_pass, self.world, self.view_entity, range)
        {
            error!("Error encountered while rendering the outline flood init phase {err:?}");
        }
    }
}
