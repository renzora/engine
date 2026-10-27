//! Transparent virtual geometry: each blended instance is an item of Bevy's
//! own transparent pass.
//!
//! Blending needs every layer behind a pixel, which a visibility buffer cannot
//! hold, so these instances are drawn directly instead: the cull pass picks
//! their clusters into a bucket per instance, and [`queue`] adds the instance
//! to Bevy's `Transparent3d` phase. Being an item there is what sorts it back
//! to front against every other transparent object, ordinary or not, exactly
//! as an ordinary mesh would be. Clusters within one instance are not sorted,
//! which is also how Bevy treats the triangles of one mesh.
//!
//! The pipeline is the material's own forward pipeline, as Bevy specializes it
//! for an ordinary mesh with this view, blend mode and cull mode, with its
//! vertex stage swapped for `transparent.wesl` and group 2 for a layout that
//! holds Bevy's `Mesh` uniforms (which the fragment shader reads) beside the
//! scene.

use std::collections::HashMap;

use bevy::camera::Camera3d;
use bevy::core_pipeline::core_3d::{Transparent3d, TransparentSortingInfo3d};
use bevy::material::key::{ErasedMaterialPipelineKey, ErasedMeshPipelineKey};
use bevy::mesh::MeshVertexBufferLayouts;
use bevy::pbr::{
    alpha_mode_pipeline_key, MaterialPipeline, MeshPipelineKey, MeshViewBindGroup, PreparedMaterial, ViewKeyCache,
};
use bevy::prelude::*;
use bevy::render::erased_render_asset::ErasedRenderAssets;
use bevy::render::material_bind_groups::MaterialBindGroupAllocators;
use bevy::render::render_phase::{
    Draw, DrawError, DrawFunctionId, DrawFunctions, PhaseItem, PhaseItemExtraIndex, TrackedRenderPass,
    ViewSortedRenderPhases,
};
use bevy::render::render_resource::*;
use bevy::render::sync_world::MainEntity;
use bevy::render::view::ExtractedView;

use super::materials::{specialize_main, vertex_layout};
use super::pipelines::{vg_bindings, VgPipelines, TRANSPARENT_VG_BASE};
use super::scene::GpuScene;
use super::view::{VgView, VgViewBindGroups, BUCKET_STRIDE};
use crate::material::VgMaterialTypes;

#[derive(Resource)]
pub(crate) struct TransparentDrawFunction(DrawFunctionId);

pub(crate) fn init(mut commands: Commands, draw_functions: Res<DrawFunctions<Transparent3d>>) {
    let id = draw_functions.write().add(DrawVgTransparent);
    commands.insert_resource(TransparentDrawFunction(id));
}

/// What one transparent instance's draw binds, per view.
pub(crate) struct TransparentDraw {
    bucket: u32,
    material: BindGroup,
}

#[derive(Component, Default)]
pub(crate) struct VgTransparentDraws(HashMap<MainEntity, TransparentDraw>);

/// Specialized pipelines, by material key and whether the mesh has tangents
/// (which decides the vertex outputs the fragment shader expects).
#[derive(Resource, Default)]
pub(crate) struct TransparentPipelines(HashMap<(ErasedMaterialPipelineKey, bool), CachedRenderPipelineId>);

#[cfg(test)]
impl TransparentPipelines {
    pub(crate) fn ids(&self) -> Vec<CachedRenderPipelineId> {
        self.0.values().copied().collect()
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn queue(
    mut commands: Commands,
    gpu: Res<GpuScene>,
    pipelines: Option<Res<VgPipelines>>,
    draw_function: Option<Res<TransparentDrawFunction>>,
    pipeline_cache: Res<PipelineCache>,
    material_pipeline: Res<MaterialPipeline>,
    render_materials: Res<ErasedRenderAssets<PreparedMaterial>>,
    allocators: Res<MaterialBindGroupAllocators>,
    mut vertex_layouts: ResMut<MeshVertexBufferLayouts>,
    view_keys: Res<ViewKeyCache>,
    types: Option<Res<VgMaterialTypes>>,
    mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    mut specialized: ResMut<TransparentPipelines>,
    views: Query<(Entity, &ExtractedView), (With<Camera3d>, With<VgView>)>,
) {
    let (Some(pipelines), Some(draw_function), Some(types)) = (pipelines, draw_function, types) else {
        return;
    };
    let layouts = [
        vertex_layout(&mut vertex_layouts, false),
        vertex_layout(&mut vertex_layouts, true),
    ];
    for (view_entity, view) in &views {
        let mut draws = VgTransparentDraws::default();
        let (Some(phase), Some(&view_key)) = (
            phases.get_mut(&view.retained_view_entity),
            view_keys.get(&view.retained_view_entity),
        ) else {
            commands.entity(view_entity).insert(draws);
            continue;
        };
        let view_key = view_key
            | MeshPipelineKey::from_primitive_topology_and_strip_index(PrimitiveTopology::TriangleList, None);

        for instance in &gpu.transparent {
            let type_id = instance.material.type_id();
            if !types.0.contains(&type_id) {
                continue;
            }
            let Some(material) = render_materials.get(instance.material) else {
                continue;
            };
            let properties = &material.properties;
            let Some(bind_group) = allocators
                .get(&type_id)
                .and_then(|a| a.get(instance.group))
                .and_then(|g| g.bind_group())
                .cloned()
            else {
                continue;
            };
            // The blend state comes from the alpha mode, which Bevy adds per
            // mesh rather than keeping in the material's own key bits.
            let mesh_key = view_key
                | properties.mesh_pipeline_key_bits.downcast()
                | alpha_mode_pipeline_key(properties.alpha_mode, &Msaa::Off);
            let key = ErasedMaterialPipelineKey {
                mesh_key: ErasedMeshPipelineKey::new(mesh_key),
                material_key: properties.material_key.clone(),
                type_id,
            };
            let cache_key = (key.clone(), instance.has_tangents);
            let pipeline = match specialized.0.get(&cache_key) {
                Some(&id) => id,
                None => {
                    let layout = &layouts[instance.has_tangents as usize];
                    let Ok(mut descriptor) = specialize_main(&material_pipeline, properties, key, layout) else {
                        continue;
                    };
                    let mut defs = descriptor.vertex.shader_defs.clone();
                    defs.extend(vg_bindings(2, TRANSPARENT_VG_BASE));
                    descriptor.label = Some("vg_transparent".into());
                    descriptor.vertex = VertexState {
                        shader: pipelines.transparent_vertex.clone(),
                        shader_defs: defs,
                        entry_point: Some("vertex".into()),
                        buffers: Vec::new(),
                        ..default()
                    };
                    // Group 2 was Bevy's mesh layout; the fragment shader's
                    // `Mesh` uniforms are at binding 0 of this one too.
                    let Some(slot) = descriptor.layout.get_mut(2) else {
                        continue;
                    };
                    *slot = pipelines.transparent_layout.clone();
                    let id = pipeline_cache.queue_render_pipeline(descriptor);
                    specialized.0.insert(cache_key, id);
                    id
                }
            };
            // The item is keyed by a made-up entity, never the mesh's own. The
            // mesh entity keeps its `Mesh3d` (shadows draw it), so Bevy's
            // batching finds it among its mesh instances and merges
            // consecutive items of one pipeline into a single draw: the first
            // item's range swallowed the rest, and only one transparent mesh
            // was ever drawn. An id from the top of the index range, which
            // the main world never hands out, is unknown to Bevy and leaves
            // the item alone. The view entity, not `Entity::PLACEHOLDER`,
            // completes the key, so it never collides with an item of Bevy's
            // own for the same mesh as it is handed back.
            let key = MainEntity::from(
                Entity::from_raw_u32(u32::MAX - 1 - instance.mesh_index).unwrap_or(Entity::PLACEHOLDER),
            );
            phase.add_transient(Transparent3d {
                sorting_info: TransparentSortingInfo3d::Sorted {
                    mesh_center: instance.center,
                    depth_bias: properties.depth_bias,
                },
                distance: 0.0,
                pipeline,
                entity: (view_entity, key),
                draw_function: draw_function.0,
                batch_range: 0..1,
                extra_index: PhaseItemExtraIndex::None,
                indexed: false,
            });
            draws.0.insert(
                key,
                TransparentDraw {
                    bucket: instance.bucket,
                    material: bind_group,
                },
            );
        }
        commands.entity(view_entity).insert(draws);
    }
}

/// Draws one transparent instance: its bucket's clusters, with its material.
struct DrawVgTransparent;

impl Draw<Transparent3d> for DrawVgTransparent {
    fn draw<'w>(
        &mut self,
        world: &'w World,
        pass: &mut TrackedRenderPass<'w>,
        view: Entity,
        item: &Transparent3d,
    ) -> Result<(), DrawError> {
        let Some(pipeline) = world.resource::<PipelineCache>().get_render_pipeline(item.pipeline) else {
            // Still compiling: skipped this frame, as Bevy skips its own.
            return Ok(());
        };
        let view = world.entity(view);
        let (Some(mesh_view), Some(vg), Some(groups), Some(draws)) = (
            view.get::<MeshViewBindGroup>(),
            view.get::<VgView>(),
            view.get::<VgViewBindGroups>(),
            view.get::<VgTransparentDraws>(),
        ) else {
            return Err(DrawError::InvalidViewQuery);
        };
        let Some(draw) = draws.0.get(&item.main_entity()).filter(|d| d.bucket < vg.bucket_count) else {
            return Ok(());
        };
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(0, &mesh_view.main, &mesh_view.main_offsets);
        pass.set_bind_group(1, &mesh_view.binding_array, &[]);
        pass.set_bind_group(2, &groups.transparent, &[draw.bucket * BUCKET_STRIDE]);
        pass.set_bind_group(3, &draw.material, &[]);
        pass.draw_indirect(&vg.masked_draws, u64::from(draw.bucket) * 16);
        Ok(())
    }
}
