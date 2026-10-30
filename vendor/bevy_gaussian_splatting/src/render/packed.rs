use bevy::render::{
    render_resource::{
        BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource,
        BindingType, Buffer, BufferBinding, BufferBindingType, BufferInitDescriptor, BufferSize,
        BufferUsages, ShaderStages,
    },
    renderer::RenderDevice,
};
use bevy_interleave::prelude::PlanarSync;

use crate::{
    gaussian::formats::planar_3d::{Gaussian3d, PlanarGaussian3d},
    render::CloudPipeline,
};

#[derive(Debug, Clone)]
pub struct PackedBuffers {
    pub gaussians: Buffer,
}

pub fn prepare_cloud(render_device: &RenderDevice, cloud: &PlanarGaussian3d) -> PackedBuffers {
    let packed: Vec<Gaussian3d> = cloud.iter().collect();
    let gaussians = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("packed_gaussian_cloud_buffer"),
        contents: bytemuck::cast_slice(packed.as_slice()),
        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST | BufferUsages::STORAGE,
    });

    PackedBuffers { gaussians }
}

pub fn get_bind_group_layout(render_device: &RenderDevice, read_only: bool) -> BindGroupLayout {
    render_device.create_bind_group_layout(
        Some("packed_gaussian_cloud_layout"),
        &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: BufferSize::new(std::mem::size_of::<Gaussian3d>() as u64),
            },
            count: None,
        }],
    )
}

#[cfg(feature = "packed")]
pub fn get_bind_group<R: PlanarSync>(
    render_device: &RenderDevice,
    gaussian_cloud_pipeline: &CloudPipeline<R>,
    cloud: &PackedBuffers,
) -> BindGroup {
    render_device.create_bind_group(
        "packed_gaussian_cloud_bind_group",
        &gaussian_cloud_pipeline.gaussian_cloud_layout,
        &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::Buffer(BufferBinding {
                buffer: &cloud.gaussians,
                offset: 0,
                size: BufferSize::new(cloud.gaussians.size()),
            }),
        }],
    )
}
