use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};

use crate::core::{EditorState, MainCamera};

use super::ViewportImage;

/// Tracks the last known viewport size to detect changes
#[derive(Resource, Default)]
pub struct ViewportTextureSize {
    pub width: u32,
    pub height: u32,
}

pub fn setup_viewport_texture(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let size = Extent3d {
        width: 1050,
        height: 881,
        depth_or_array_layers: 1,
    };

    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("viewport_texture"),
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bgra8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
        ..default()
    };
    image.resize(size);

    let image_handle = images.add(image);
    commands.insert_resource(ViewportImage(image_handle));
    commands.insert_resource(ViewportTextureSize {
        width: size.width,
        height: size.height,
    });
}

/// System to resize the viewport texture when the UI viewport size changes
pub fn resize_viewport_texture(
    editor_state: Res<EditorState>,
    mut texture_size: ResMut<ViewportTextureSize>,
    viewport_image: Res<ViewportImage>,
    mut images: ResMut<Assets<Image>>,
    mut camera_query: Query<&mut Projection, With<MainCamera>>,
) {
    // Skip if viewport size hasn't been set yet by UI (still at default 0,0)
    if editor_state.viewport_size[0] < 10.0 || editor_state.viewport_size[1] < 10.0 {
        return;
    }

    // Get the current viewport size from UI
    let new_width = editor_state.viewport_size[0] as u32;
    let new_height = editor_state.viewport_size[1] as u32;

    // Check if size has changed
    if new_width == texture_size.width && new_height == texture_size.height {
        return;
    }

    // Update the tracked size
    texture_size.width = new_width;
    texture_size.height = new_height;

    // Resize the render texture
    if let Some(image) = images.get_mut(&viewport_image.0) {
        let new_size = Extent3d {
            width: new_width,
            height: new_height,
            depth_or_array_layers: 1,
        };
        image.resize(new_size);
    }

    // Update the camera's aspect ratio
    let aspect = new_width as f32 / new_height as f32;
    for mut projection in camera_query.iter_mut() {
        if let Projection::Perspective(ref mut persp) = *projection {
            persp.aspect_ratio = aspect;
        }
    }
}
