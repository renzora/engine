use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::prepass::MotionVectorPrepass;
use bevy::ecs::change_detection::Tick;
use bevy::ecs::system::SystemChangeTick;
use bevy::math::Mat3A;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::render::mesh::allocator::MeshAllocator;
use bevy::render::mesh::RenderMesh;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_phase::{
    BinnedRenderPhaseType, DrawFunctions, InputUniformIndex, PhaseItemExtraIndex,
    ViewBinnedRenderPhases, ViewRangefinder3d, ViewSortedRenderPhases,
};
use bevy::render::render_resource::{
    CachedRenderPipelineId, PipelineCache, SpecializedMeshPipelines, TextureFormat,
};
use bevy::render::sync_world::{MainEntity, MainEntityHashMap};
use bevy::render::view::{ExtractedView, RetainedViewEntity};
use bevy::render::Extract;

use crate::node::{
    OpaqueOutline, OutlineBatchSetKey, OutlineBinKey, StencilOutline, TransparentOutline,
};
use crate::pipeline_key::ComputedOutlineKey;
use crate::pipeline_key::{DerivedPipelineKey, EntityPipelineKey, PassType, ViewPipelineKey};
use crate::ComputedOutline;
use crate::{
    pipeline::OutlinePipeline,
    render::DrawOutline,
    uniforms::{DrawMode, ExtractedOutline},
    view_uniforms::OutlineQueueStatus,
};

#[derive(Resource, Default)]
pub struct OutlineEntitiesChanged {
    entities: Vec<MainEntity>,
}

#[derive(Resource, Default)]
pub struct OutlineCache {
    pub(crate) view_map: HashMap<RetainedViewEntity, OutlineViewCache>,
}

#[derive(Default)]
pub struct OutlineViewCache {
    // Bevy 0.19 removed `ViewSpecializationTicks` (the resource that told us when
    // a view's pipeline-relevant settings changed). We instead remember the
    // view's last `ViewPipelineKey` and re-specialize when it differs — a direct
    // check of the thing that actually matters, rather than the old tick proxy.
    pub(crate) view_key: Option<ViewPipelineKey>,
    pub(crate) entity_map: MainEntityHashMap<OutlineCacheEntry>,
}

#[derive(Debug, Clone, Copy)]
pub struct OutlineCacheEntry {
    pub(crate) changed_tick: Tick,
    pub(crate) stencil_pipeline_id: CachedRenderPipelineId,
    pub(crate) volume_pipeline_id: CachedRenderPipelineId,
}

pub(crate) struct OutlineRangefinder {
    rangefinder: ViewRangefinder3d,
    world_from_view: Mat3A,
}

impl OutlineRangefinder {
    pub(crate) fn new(view: &ExtractedView) -> Self {
        Self {
            rangefinder: view.rangefinder3d(),
            world_from_view: view.world_from_view.affine().matrix3,
        }
    }

    pub(crate) fn distance_of(&self, outline: &ExtractedOutline) -> f32 {
        let world_plane = outline.instance_data.world_plane_origin
            + self.world_from_view.mul_vec3(-Vec3::Z) * outline.instance_data.world_plane_offset;
        self.rangefinder.distance(&world_plane)
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn check_outline_entities_changed(
    changed_entities: Query<Entity, Or<(Changed<ComputedOutline>, Changed<ComputedOutlineKey>)>>,
    mut entities_changed: ResMut<OutlineEntitiesChanged>,
) {
    entities_changed.entities.clear();
    for entity in changed_entities.iter() {
        entities_changed.entities.push(entity.into());
    }
}

pub(crate) fn extract_outline_entities_changed(
    entities_changed: Extract<Res<OutlineEntitiesChanged>>,
    mut entities_removed: Extract<RemovedComponents<ComputedOutline>>,
    mut outline_cache: ResMut<OutlineCache>,
) {
    for outline_view_cache in outline_cache.view_map.values_mut() {
        for entity in entities_changed.entities.iter() {
            outline_view_cache.entity_map.remove(entity);
        }
        for entity in entities_removed.read() {
            outline_view_cache
                .entity_map
                .remove(&MainEntity::from(entity));
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn specialise_outlines(
    render_meshes: Res<RenderAssets<RenderMesh>>,
    mut outline_cache: ResMut<OutlineCache>,
    mut pipelines: ResMut<SpecializedMeshPipelines<OutlinePipeline>>,
    mut all_views: Local<HashSet<RetainedViewEntity>>,
    mut warm_up_keys: Local<Vec<EntityPipelineKey>>,
    outline_pipeline: Res<OutlinePipeline>,
    pipeline_cache: Res<PipelineCache>,
    ticks: SystemChangeTick,
    views: Query<(
        &ExtractedView,
        Has<MotionVectorPrepass>,
        &Msaa,
        Option<&RenderLayers>,
    )>,
    outlines: Query<(&MainEntity, &ExtractedOutline)>,
) {
    all_views.clear();

    for (view, motion_vector_prepass, msaa, view_mask) in &views {
        all_views.insert(view.retained_view_entity);

        // Bevy 0.19 doesn't extract the `Hdr` marker into the render world, so a
        // `Has<Hdr>` query is always false and would specialize an `Rgba8UnormSrgb`
        // pipeline for an HDR (`Rgba16Float`) view — a fatal attachment mismatch.
        // Read the view's real render-target format instead.
        let hdr = view.target_format == TextureFormat::Rgba16Float;

        let view_key = ViewPipelineKey::new()
            .with_msaa(*msaa)
            .with_hdr_format(hdr)
            .with_motion_vector_prepass(motion_vector_prepass);

        let outline_view_cache = outline_cache
            .view_map
            .entry(view.retained_view_entity)
            .or_default();
        // Re-specialize the whole view when its pipeline key changes (MSAA / HDR
        // / motion-vector prepass) — replaces the old `ViewSpecializationTicks`
        // newer-than-tick check.
        if outline_view_cache.view_key != Some(view_key) {
            outline_view_cache.view_key = Some(view_key);
            outline_view_cache.entity_map.clear();
        }
        let view_layers = view_mask.unwrap_or_default();

        for (main_entity, outline) in outlines.iter() {
            if outline_view_cache.entity_map.contains_key(main_entity) {
                continue; // Already in entity cache
            };

            let enable_stencil = outline.stencil || outline.warm_up.disabled_stencil;
            let enable_volume = outline.volume || outline.warm_up.disabled_volume;
            if !enable_stencil && !enable_volume {
                continue; // Neither stencil nor volume enabled
            }

            if !outline.layers.intersects(view_layers)
                && !outline.warm_up.layers.intersects(view_layers)
            {
                continue; // Layer not enabled
            }

            let Some(mesh) = render_meshes.get(outline.mesh_id) else {
                continue; // Mesh not found
            };

            warm_up_keys.clear();
            warm_up_keys.push(outline.pipeline_key);

            if outline.warm_up.transparency {
                let range = 0..warm_up_keys.len();
                for i in range {
                    let key = warm_up_keys[i];
                    warm_up_keys.push(key.with_transparent(!key.transparent()));
                }
            }

            if outline.warm_up.vertex_offsets {
                let range = 0..warm_up_keys.len();
                for i in range {
                    let key = warm_up_keys[i];
                    warm_up_keys.push(
                        key.with_vertex_offset_zero(!key.vertex_offset_zero())
                            .with_stencil_vertex_offset_zero(!key.stencil_vertex_offset_zero()),
                    );
                }
            }

            let mut first_key = true;
            for warm_up_key in warm_up_keys.iter() {
                // Specialise stencil pipeline
                let stencil_pipeline_id = if enable_stencil {
                    let stencil_key =
                        DerivedPipelineKey::new(view_key, *warm_up_key, PassType::Stencil);

                    match pipelines.specialize(
                        &pipeline_cache,
                        &outline_pipeline,
                        stencil_key,
                        &mesh.layout,
                    ) {
                        Ok(pipeline_id) => pipeline_id,
                        Err(err) => {
                            error!("Failed to specialise stencil pipeline: {}", err);
                            CachedRenderPipelineId::INVALID
                        }
                    }
                } else {
                    CachedRenderPipelineId::INVALID
                };

                // Specialise volume pipeline
                let volume_pipeline_id = if enable_volume {
                    let pass_type = match outline.draw_mode {
                        DrawMode::Extrude => PassType::Volume,
                        #[cfg(feature = "flood")]
                        DrawMode::JumpFlood => PassType::FloodInit,
                    };
                    let volume_key = DerivedPipelineKey::new(view_key, *warm_up_key, pass_type);

                    match pipelines.specialize(
                        &pipeline_cache,
                        &outline_pipeline,
                        volume_key,
                        &mesh.layout,
                    ) {
                        Ok(pipeline_id) => pipeline_id,
                        Err(err) => {
                            error!("Failed to specialise volume pipeline: {}", err);
                            CachedRenderPipelineId::INVALID
                        }
                    }
                } else {
                    CachedRenderPipelineId::INVALID
                };

                if first_key {
                    outline_view_cache.entity_map.insert(
                        *main_entity,
                        OutlineCacheEntry {
                            changed_tick: ticks.this_run(),
                            stencil_pipeline_id,
                            volume_pipeline_id,
                        },
                    );
                    first_key = false;
                }
            }
        }
    }

    // Delete specialized pipelines belonging to views that have expired.
    outline_cache
        .view_map
        .retain(|retained_view_entity, _| all_views.contains(retained_view_entity));
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn queue_outline_mesh(
    stencil_draw_functions: Res<DrawFunctions<StencilOutline>>,
    opaque_draw_functions: Res<DrawFunctions<OpaqueOutline>>,
    transparent_draw_functions: Res<DrawFunctions<TransparentOutline>>,
    mesh_allocator: Res<MeshAllocator>,
    outline_cache: Res<OutlineCache>,
    mut stencil_phases: ResMut<ViewBinnedRenderPhases<StencilOutline>>,
    mut opaque_phases: ResMut<ViewBinnedRenderPhases<OpaqueOutline>>,
    mut transparent_phases: ResMut<ViewSortedRenderPhases<TransparentOutline>>,
    mut views: Query<(
        &ExtractedView,
        Option<&RenderLayers>,
        &mut OutlineQueueStatus,
    )>,
    outlines: Query<(Entity, &MainEntity, &ExtractedOutline)>,
) {
    let draw_stencil = stencil_draw_functions
        .read()
        .get_id::<DrawOutline>()
        .unwrap();
    let draw_opaque_outline = opaque_draw_functions
        .read()
        .get_id::<DrawOutline>()
        .unwrap();
    let draw_transparent_outline = transparent_draw_functions
        .read()
        .get_id::<DrawOutline>()
        .unwrap();

    for (view, view_mask, mut queue_status) in views.iter_mut() {
        let view_mask = view_mask.unwrap_or_default();
        let rangefinder = OutlineRangefinder::new(view);

        // The outline cache populates per-view entries lazily. When an
        // entity (e.g. the World Environment) is deleted mid-frame Bevy
        // tears down render-world views before the cache catches up,
        // leaving a brief window where this lookup misses. The phase
        // lookups just below use the same `let ... else { continue }`
        // pattern; mirror it here so deletes don't panic.
        let Some(outline_view_cache) = outline_cache.view_map.get(&view.retained_view_entity)
        else {
            continue;
        };

        let (Some(stencil_phase), Some(opaque_phase), Some(transparent_phase)) = (
            stencil_phases.get_mut(&view.retained_view_entity),
            opaque_phases.get_mut(&view.retained_view_entity),
            transparent_phases.get_mut(&view.retained_view_entity),
        ) else {
            continue; // No render phase
        };

        for (render_entity, main_entity, outline) in outlines.iter() {
            if !view_mask.intersects(&outline.layers) {
                continue; // Layer not enabled
            }

            // Bevy 0.19: `mesh_slabs` now returns `Option<MeshSlabs>` (a struct
            // bundling vertex + optional index slab) instead of a
            // `(Option<SlabId>, Option<SlabId>)` tuple.
            let mesh_slabs = mesh_allocator
                .mesh_slabs(&outline.mesh_id)
                .unwrap_or_default();
            let phase_type = if outline.automatic_batching {
                BinnedRenderPhaseType::BatchableMesh
            } else {
                BinnedRenderPhaseType::UnbatchableMesh
            };

            // Bevy 0.19: the binned/sorted phases own their per-frame retention
            // (`prepare_for_new_frame` drains transient items; re-`add` is an
            // idempotent map insert). So the old `validate_cached_entity` skip +
            // per-entity tick are gone — we simply (re)add every visible outlined
            // entity each frame. `OutlineCacheEntry.changed_tick` is no longer
            // consulted here.
            let Some(OutlineCacheEntry {
                stencil_pipeline_id,
                volume_pipeline_id,
                ..
            }) = outline_view_cache.entity_map.get(main_entity)
            else {
                continue;
            };

            // Queue stencil pass if needed
            if outline.stencil {
                stencil_phase.add(
                    OutlineBatchSetKey {
                        pipeline: *stencil_pipeline_id,
                        draw_function: draw_stencil,
                        slabs: mesh_slabs,
                    },
                    OutlineBinKey {
                        asset_id: outline.mesh_id,
                        texture_id: outline.alpha_mask_id,
                    },
                    (render_entity, *main_entity),
                    InputUniformIndex::default(),
                    phase_type,
                );
            }

            // Queue volume pass if needed
            if outline.volume && outline.draw_mode == DrawMode::Extrude {
                queue_status.has_volume = true;
                let transparent = outline.instance_data.volume_colour[3] < 1.0;

                if transparent {
                    let distance = rangefinder.distance_of(outline);
                    // Sorted phase: `add` → `add_transient` (cleared each frame).
                    transparent_phase.add_transient(TransparentOutline {
                        entity: render_entity,
                        main_entity: *main_entity,
                        pipeline: *volume_pipeline_id,
                        draw_function: draw_transparent_outline,
                        distance,
                        batch_range: 0..0,
                        extra_index: PhaseItemExtraIndex::None,
                        indexed: mesh_slabs.index_slab_id.is_some(),
                    });
                } else {
                    opaque_phase.add(
                        OutlineBatchSetKey {
                            pipeline: *volume_pipeline_id,
                            draw_function: draw_opaque_outline,
                            slabs: mesh_slabs,
                        },
                        OutlineBinKey {
                            asset_id: outline.mesh_id,
                            texture_id: outline.alpha_mask_id,
                        },
                        (render_entity, *main_entity),
                        InputUniformIndex::default(),
                        phase_type,
                    );
                }
            }
        }
    }
}
