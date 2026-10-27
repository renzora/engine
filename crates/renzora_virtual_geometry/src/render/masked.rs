//! The masked raster pass's pipelines and draws: one draw per bucket (see
//! `MaskedBucket`), each with the bind group that holds its materials.
//!
//! The pipeline's group 2 is the material's own bind group, which is why it is
//! specialized from a material's layout rather than built once at startup: one
//! per material type, bindless or not, culled or not.

use std::collections::HashMap;

use bevy::core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy::pbr::PreparedMaterial;
use bevy::prelude::*;
use bevy::render::erased_render_asset::ErasedRenderAssets;
use bevy::render::material_bind_groups::MaterialBindGroupAllocators;
use bevy::render::render_resource::*;
use bevy::shader::ShaderDefVal;

use super::pipelines::VgPipelines;
use super::scene::GpuScene;
use crate::material::VgMaterialTypes;

/// The group a masked pipeline puts the material's bind group in. Not Bevy's
/// usual 3: groups 0 and 1 are the raster pass's own and 3 holds the bucket.
/// A material's layout does not depend on the group it is bound at, and the
/// shader names its group through `MATERIAL_BIND_GROUP`, so any number works.
const MASKED_MATERIAL_GROUP: u32 = 2;

pub(crate) struct MaskedDraw {
    pub pipeline: CachedRenderPipelineId,
    pub material: BindGroup,
}

/// One entry per `GpuScene::masked` bucket, in the same order. `None` when a
/// bucket's material is not ready yet: its clusters are culled but not drawn
/// that frame.
#[derive(Resource, Default)]
pub(crate) struct MaskedDraws(pub Vec<Option<MaskedDraw>>);

pub(crate) fn queue(
    gpu: Res<GpuScene>,
    pipelines: Option<Res<VgPipelines>>,
    pipeline_cache: Res<PipelineCache>,
    render_materials: Res<ErasedRenderAssets<PreparedMaterial>>,
    allocators: Res<MaterialBindGroupAllocators>,
    mut draws: ResMut<MaskedDraws>,
    mut cache: Local<HashMap<(std::any::TypeId, bool, bool), CachedRenderPipelineId>>,
    types: Option<Res<VgMaterialTypes>>,
) {
    draws.0.clear();
    let (Some(pipelines), Some(types)) = (pipelines, types) else {
        return;
    };
    for bucket in &gpu.masked {
        let draw = (|| {
            // The masked raster pass reads `StandardMaterial`'s bindings,
            // which every registered type starts with (see `material.rs`).
            if !types.0.contains(&bucket.material.type_id()) {
                return None;
            }
            let material = render_materials.get(bucket.material)?;
            let properties = &material.properties;
            let bind_group = allocators
                .get(&bucket.material.type_id())?
                .get(bucket.group)?
                .bind_group()?
                .clone();
            let material_layout = properties.material_layout.clone()?;
            // Per type as well: each material type has its own bind group
            // layout, which the pipeline is built against.
            let key = (bucket.material.type_id(), properties.bindless, bucket.double_sided);
            let pipeline = *cache.entry(key).or_insert_with(|| {
                let mut defs = super::pipelines::vg_bindings(1, 0).to_vec();
                defs.push(ShaderDefVal::UInt("MATERIAL_BIND_GROUP".into(), MASKED_MATERIAL_GROUP));
                if properties.bindless {
                    defs.push("BINDLESS".into());
                }
                pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
                    label: Some("vg_raster_masked".into()),
                    layout: vec![
                        pipelines.raster_view_layout.clone(),
                        pipelines.masked_scene_layout.clone(),
                        material_layout,
                        pipelines.bucket_layout.clone(),
                    ],
                    vertex: VertexState {
                        shader: pipelines.raster_masked.clone(),
                        shader_defs: defs.clone(),
                        ..default()
                    },
                    primitive: PrimitiveState {
                        topology: PrimitiveTopology::TriangleList,
                        front_face: FrontFace::Ccw,
                        cull_mode: if bucket.double_sided { None } else { Some(Face::Back) },
                        ..default()
                    },
                    depth_stencil: Some(DepthStencilState {
                        format: CORE_3D_DEPTH_FORMAT,
                        depth_write_enabled: Some(true),
                        depth_compare: Some(CompareFunction::GreaterEqual),
                        stencil: default(),
                        bias: default(),
                    }),
                    fragment: Some(FragmentState {
                        shader: pipelines.raster_masked.clone(),
                        shader_defs: defs,
                        targets: vec![Some(ColorTargetState {
                            format: TextureFormat::R32Uint,
                            blend: None,
                            write_mask: ColorWrites::ALL,
                        })],
                        ..default()
                    }),
                    ..default()
                })
            });
            Some(MaskedDraw {
                pipeline,
                material: bind_group,
            })
        })();
        draws.0.push(draw);
    }
}

#[cfg(test)]
impl MaskedDraws {
    pub(crate) fn ids(&self) -> Vec<CachedRenderPipelineId> {
        self.0.iter().flatten().map(|d| d.pipeline).collect()
    }
}
