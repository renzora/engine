use bevy::math::Affine3A;
use bevy::prelude::*;
use bevy::render::batching::gpu_preprocessing::GpuPreprocessingMode;
use bevy::render::extract_component::ComponentUniforms;
use bevy::render::render_phase::{ViewBinnedRenderPhases, ViewSortedRenderPhases};
use bevy::render::render_resource::ShaderType;
use bevy::render::render_resource::{BindGroup, BindGroupEntry};
use bevy::render::renderer::RenderDevice;
use bevy::render::sync_world::RenderEntity;
use bevy::render::view::RetainedViewEntity;
use bevy::render::Extract;

use crate::node::{OpaqueOutline, StencilOutline, TransparentOutline};
use crate::pipeline::OutlinePipeline;

#[derive(Clone, Component, ShaderType)]
pub(crate) struct OutlineViewUniform {
    pub clip_from_world: Mat4,
    pub world_from_view_a: [Vec4; 2],
    pub world_from_view_b: f32,
    pub aspect: f32,
    pub scale_clip_from_logical: Vec2,
    pub scale_physical_from_logical: f32,
}

#[derive(Resource)]
pub(crate) struct OutlineViewBindGroup {
    pub(crate) bind_group: BindGroup,
}

#[derive(Component, Default)]
pub(crate) struct OutlineQueueStatus {
    pub(crate) has_volume: bool,
}

#[allow(clippy::type_complexity)]
pub(crate) fn extract_outline_view_uniforms(
    mut commands: Commands,
    mut stencil_phases: ResMut<ViewBinnedRenderPhases<StencilOutline>>,
    mut opaque_phases: ResMut<ViewBinnedRenderPhases<OpaqueOutline>>,
    mut transparent_phases: ResMut<ViewSortedRenderPhases<TransparentOutline>>,
    query: Extract<Query<(Entity, &RenderEntity, &Camera, &GlobalTransform), With<Camera3d>>>,
) {
    fn transpose_3x3(m: &Affine3A) -> ([Vec4; 2], f32) {
        let transpose_3x3 = m.matrix3.transpose();
        (
            [
                (transpose_3x3.x_axis, transpose_3x3.y_axis.x).into(),
                (transpose_3x3.y_axis.yz(), transpose_3x3.z_axis.xy()).into(),
            ],
            transpose_3x3.z_axis.z,
        )
    }

    for (main_entity, entity, camera, transform) in query.iter() {
        if !camera.is_active {
            continue;
        }
        if let Some(size) = camera.logical_viewport_size() {
            let view_from_world = transform.to_matrix().inverse();
            let (world_from_view_a, world_from_view_b) = transpose_3x3(&transform.affine());
            commands
                .entity(entity.id())
                .insert(OutlineViewUniform {
                    clip_from_world: camera.clip_from_view() * view_from_world,
                    world_from_view_a,
                    world_from_view_b,
                    aspect: size.x / size.y,
                    scale_clip_from_logical: 2.0 / size,
                    scale_physical_from_logical: camera.target_scaling_factor().unwrap_or(1.0),
                })
                .insert(OutlineQueueStatus::default());

            let retained_view_entity = RetainedViewEntity::new(main_entity.into(), None, 0);
            stencil_phases.prepare_for_new_frame(retained_view_entity, GpuPreprocessingMode::None);
            opaque_phases.prepare_for_new_frame(retained_view_entity, GpuPreprocessingMode::None);
            transparent_phases.insert_or_clear(retained_view_entity);
        }
    }
}

pub(crate) fn prepare_outline_view_bind_group(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    outline_pipeline: Res<OutlinePipeline>,
    view_uniforms: Res<ComponentUniforms<OutlineViewUniform>>,
) {
    if let Some(view_binding) = view_uniforms.binding() {
        let bind_group = render_device.create_bind_group(
            "outline_view_bind_group",
            &outline_pipeline.outline_view_bind_group_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: view_binding.clone(),
            }],
        );
        commands.insert_resource(OutlineViewBindGroup { bind_group });
    }
}
