use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};

use crate::console_info;
use crate::core::{MainCamera, ViewportState};

use super::ViewportImage;

/// Returns the texture format and usages for the viewport texture.
/// When compiled with the `solari` feature, uses HDR format (Rgba16Float + STORAGE_BINDING)
/// so the Solari-modified render pipeline works from startup without needing a texture swap.
/// Standard PBR rendering also works fine with HDR (tone mapping handles conversion).
fn viewport_texture_config() -> (TextureFormat, TextureUsages) {
    #[cfg(feature = "solari")]
    {
        (
            TextureFormat::Rgba16Float,
            TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::STORAGE_BINDING,
        )
    }
    #[cfg(not(feature = "solari"))]
    {
        (
            TextureFormat::Bgra8UnormSrgb,
            TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT,
        )
    }
}

/// Tracks the last known viewport size to detect changes
#[derive(Resource, Default)]
pub struct ViewportTextureSize {
    pub width: u32,
    pub height: u32,
}

pub fn setup_viewport_texture(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    console_info!("Viewport", "=== SETUP VIEWPORT TEXTURE ===");

    let size = Extent3d {
        width: 1050,
        height: 881,
        depth_or_array_layers: 1,
    };

    let (format, usage) = viewport_texture_config();

    console_info!("Viewport", "Initial size: {}x{}", size.width, size.height);
    console_info!("Viewport", "Format: {:?}", format);
    console_info!("Viewport", "Usage: {:?}", usage);

    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("viewport_texture"),
            size,
            dimension: TextureDimension::D2,
            format,
            mip_level_count: 1,
            sample_count: 1,
            usage,
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

    console_info!("Viewport", "=== VIEWPORT TEXTURE READY ===");
}

/// System to resize the viewport texture when the UI viewport size changes
pub fn resize_viewport_texture(
    viewport_state: Res<ViewportState>,
    mut texture_size: ResMut<ViewportTextureSize>,
    viewport_image: Res<ViewportImage>,
    mut images: ResMut<Assets<Image>>,
    mut camera_query: Query<&mut Projection, With<MainCamera>>,
) {
    // Skip if viewport size hasn't been set yet by UI (still at default 0,0)
    if viewport_state.size[0] < 10.0 || viewport_state.size[1] < 10.0 {
        return;
    }

    // Get the current viewport size from UI
    let new_width = viewport_state.size[0] as u32;
    let new_height = viewport_state.size[1] as u32;

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
