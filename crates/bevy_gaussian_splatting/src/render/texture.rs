use bevy::{
    prelude::*,
    render::{
        render_resource::{
            BindGroupLayout, BindGroupLayoutEntry, BindingType, ShaderStages, TextureSampleType,
            TextureViewDimension,
        },
        renderer::RenderDevice,
    },
};
use static_assertions::assert_cfg;

assert_cfg!(
    feature = "planar",
    "texture rendering is only supported with the `planar` feature enabled",
);

/// Placeholder plugin for the buffer-texture render path.
///
/// Cloud data currently stays on the canonical planar storage path; the
/// texture-specific bindings are used for sorted-entry indirection only.
#[derive(Default)]
pub struct BufferTexturePlugin;

impl Plugin for BufferTexturePlugin {
    fn build(&self, _app: &mut App) {}
}

pub fn get_sorted_bind_group_layout(render_device: &RenderDevice) -> BindGroupLayout {
    render_device.create_bind_group_layout(
        Some("texture_sorted_layout"),
        &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE,
            ty: BindingType::Texture {
                view_dimension: TextureViewDimension::D2,
                sample_type: TextureSampleType::Uint,
                multisampled: false,
            },
            count: None,
        }],
    )
}
