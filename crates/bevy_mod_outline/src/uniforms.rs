use bevy::{
    math::Affine3,
    mesh::skinning::SkinnedMesh,
    pbr::SkinUniforms,
    platform::collections::HashMap,
    prelude::*,
    camera::visibility::RenderLayers,
    render::{
        batching::{no_gpu_preprocessing::BatchedInstanceBuffer, NoAutomaticBatching},
        mesh::allocator::MeshAllocator,
        render_asset::RenderAssets,
        render_resource::{BindGroup, BindGroupEntries, BindGroupEntry, ShaderType},
        renderer::RenderDevice,
        sync_world::{MainEntity, MainEntityHashMap, RenderEntity},
        texture::{FallbackImage, GpuImage},
        Extract,
    },
};

use crate::{
    pipeline::OutlinePipeline,
    pipeline_key::{ComputedOutlineKey, EntityPipelineKey},
    ComputedOutline, OutlineWarmUp,
};

#[derive(Clone, Component)]
pub struct ExtractedOutline {
    pub(crate) stencil: bool,
    pub(crate) volume: bool,
    pub(crate) draw_mode: DrawMode,
    pub(crate) layers: RenderLayers,
    pub(crate) mesh_id: AssetId<Mesh>,
    pub(crate) alpha_mask_id: Option<AssetId<Image>>,
    pub(crate) pipeline_key: EntityPipelineKey,
    pub(crate) automatic_batching: bool,
    pub(crate) instance_data: OutlineInstanceUniform,
    pub(crate) warm_up: OutlineWarmUp,
}

#[derive(Resource, Default)]
pub struct RenderOutlineInstances {
    entity_map: MainEntityHashMap<ExtractedOutline>,
}

impl RenderOutlineInstances {
    pub fn get(&self, main_entity: &MainEntity) -> Option<&ExtractedOutline> {
        self.entity_map.get(main_entity)
    }
}

#[derive(Clone, ShaderType)]
pub(crate) struct OutlineInstanceUniform {
    pub world_from_local: [Vec4; 3],
    pub world_plane_origin: Vec3,
    pub world_plane_offset: Vec3,
    pub volume_colour: Vec4,
    pub volume_offset: f32,
    pub stencil_offset: f32,
    pub alpha_mask_threshold: f32,
    pub first_vertex_index: u32,
    pub current_skin_index: u32,
}

impl OutlineInstanceUniform {
    pub(crate) fn prepare_instance(
        &self,
        mesh_id: &AssetId<Mesh>,
        main_entity: MainEntity,
        mesh_allocator: &MeshAllocator,
        skin_uniforms: &SkinUniforms,
    ) -> Self {
        let mut instance_data = self.clone();
        instance_data.first_vertex_index = mesh_allocator
            .mesh_vertex_slice(mesh_id)
            .map(|x| x.range.start)
            .unwrap_or(0);
        instance_data.current_skin_index = skin_uniforms.skin_index(main_entity).unwrap_or(0);
        instance_data
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum DepthMode {
    Flat = 1,
    Real = 2,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum DrawMode {
    Extrude = 1,
    #[cfg(feature = "flood")]
    JumpFlood = 2,
}

#[derive(Resource)]
pub(crate) struct OutlineInstanceBindGroup {
    pub bind_group: BindGroup,
}

pub(crate) fn set_outline_visibility(
    mut query: Query<(&mut ViewVisibility, &ComputedOutline)>,
) {
    use bevy::camera::visibility::SetViewVisibility;
    for (mut visibility, computed) in query.iter_mut() {
        if let ComputedOutline(Some(computed)) = computed {
            if computed.volume.value.enabled || computed.stencil.value.enabled {
                visibility.set_visible();
            }
        }
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn extract_outlines(
    mut commands: Commands,
    mut render_outlines: ResMut<RenderOutlineInstances>,
    outlines: Extract<
        Query<(
            Entity,
            RenderEntity,
            &ComputedOutline,
            &ComputedOutlineKey,
            &GlobalTransform,
            &Mesh3d,
            Has<NoAutomaticBatching>,
            Has<SkinnedMesh>,
        )>,
    >,
) {
    render_outlines.entity_map.clear();

    for (entity, render_entity, computed, key, transform, mesh, no_automatic_batching, is_skinned) in
        outlines.iter()
    {
        // Skip skinned meshes - outline pipeline doesn't support skinned mesh bind group layout
        if is_skinned {
            continue;
        }
        let ComputedOutline(Some(computed)) = computed else {
            continue;
        };
        let extracted_outline = ExtractedOutline {
            stencil: computed.stencil.value.enabled,
            volume: computed.volume.value.enabled,
            draw_mode: computed.mode.value.draw_mode,
            layers: computed.layers.value.clone(),
            mesh_id: mesh.id(),
            alpha_mask_id: computed
                .alpha_mask
                .value
                .texture
                .as_ref()
                .map(|texture| texture.id()),
            pipeline_key: key.0,
            automatic_batching: !no_automatic_batching
                && computed.mode.value.draw_mode == DrawMode::Extrude,
            instance_data: OutlineInstanceUniform {
                world_from_local: Affine3::from(&transform.affine()).to_transpose(),
                world_plane_origin: computed.depth.value.world_plane_origin,
                world_plane_offset: computed.depth.value.world_plane_offset,
                stencil_offset: computed.stencil.value.offset,
                volume_offset: computed.volume.value.offset,
                volume_colour: computed.volume.value.colour.to_vec4(),
                alpha_mask_threshold: computed.alpha_mask.value.threshold,
                first_vertex_index: 0,
                current_skin_index: 0,
            },
            warm_up: computed.warm_up.value.clone(),
        };
        commands
            .entity(render_entity)
            .insert(extracted_outline.clone());
        render_outlines
            .entity_map
            .insert(entity.into(), extracted_outline);
    }
}

pub(crate) fn prepare_outline_instance_bind_group(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    batched_instance_buffer: Res<BatchedInstanceBuffer<OutlineInstanceUniform>>,
    outline_pipeline: Res<OutlinePipeline>,
) {
    if let Some(instance_binding) = batched_instance_buffer.instance_data_binding() {
        let bind_group = render_device.create_bind_group(
            "outline_instance_mesh_bind_group",
            &outline_pipeline.outline_instance_bind_group_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: instance_binding,
            }],
        );
        commands.insert_resource(OutlineInstanceBindGroup { bind_group });
    };
}

#[derive(Resource)]
pub(crate) struct AlphaMaskBindGroups {
    pub bind_groups: HashMap<AssetId<Image>, BindGroup>,
    pub default_bind_group: BindGroup,
}

impl FromWorld for AlphaMaskBindGroups {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let fallback_image = world.resource::<FallbackImage>();
        let outline_pipeline = world.resource::<OutlinePipeline>();

        Self {
            bind_groups: HashMap::new(),
            default_bind_group: render_device.create_bind_group(
                "default_outline_alpha_mask_bind_group",
                &outline_pipeline.alpha_mask_bind_group_layout,
                &BindGroupEntries::sequential((
                    &fallback_image.d2.texture_view,
                    &fallback_image.d2.sampler,
                )),
            ),
        }
    }
}

pub(crate) fn prepare_alpha_mask_bind_groups(
    mut alpha_mask_bind_groups: ResMut<AlphaMaskBindGroups>,
    render_device: Res<RenderDevice>,
    outline_pipeline: Res<OutlinePipeline>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    outlines: Query<&ExtractedOutline>,
) {
    alpha_mask_bind_groups.bind_groups.clear();

    // Collect all unique textures used by outline alpha masks
    for outline in outlines.iter() {
        if let Some(texture_id) = outline.alpha_mask_id {
            if let Some(gpu_image) = gpu_images.get(texture_id) {
                alpha_mask_bind_groups
                    .bind_groups
                    .entry(texture_id)
                    .or_insert_with(|| {
                        render_device.create_bind_group(
                            "outline_alpha_mask_bind_group",
                            &outline_pipeline.alpha_mask_bind_group_layout,
                            &BindGroupEntries::sequential((
                                &gpu_image.texture_view,
                                &gpu_image.sampler,
                            )),
                        )
                    });
            }
        }
    }
}
