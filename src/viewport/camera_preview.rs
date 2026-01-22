//! Camera preview system - renders from selected camera's viewpoint

use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};

use crate::core::EditorState;
use crate::gizmo::preview_camera_layers;
use crate::node_system::CameraNodeData;
use crate::scene::EditorOnly;

/// Resource holding the camera preview render texture
#[derive(Resource)]
pub struct CameraPreviewImage(pub Handle<Image>);

/// Marker component for the camera preview camera
#[derive(Component)]
pub struct CameraPreviewMarker;

/// Preview image size (kept small for performance)
const PREVIEW_WIDTH: u32 = 320;
const PREVIEW_HEIGHT: u32 = 180;

/// Sets up the camera preview render texture
pub fn setup_camera_preview_texture(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let size = Extent3d {
        width: PREVIEW_WIDTH,
        height: PREVIEW_HEIGHT,
        depth_or_array_layers: 1,
    };

    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("camera_preview_texture"),
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
    commands.insert_resource(CameraPreviewImage(image_handle));
}

/// System that manages the camera preview - spawns/updates/despawns the preview camera
/// based on whether a Camera3D node is selected
pub fn update_camera_preview(
    mut commands: Commands,
    editor_state: Res<EditorState>,
    camera_preview_image: Res<CameraPreviewImage>,
    camera_nodes: Query<(&Transform, &CameraNodeData), Without<CameraPreviewMarker>>,
    mut preview_camera: Query<
        (Entity, &mut Transform, &mut Projection),
        With<CameraPreviewMarker>,
    >,
) {
    // Check if a camera node is selected
    let selected_camera = editor_state
        .selected_entity
        .and_then(|entity| camera_nodes.get(entity).ok());

    let existing_preview = preview_camera.iter_mut().next();

    match (selected_camera, existing_preview) {
        // Camera selected and preview camera exists - update it
        (Some((camera_transform, camera_data)), Some((_, mut preview_transform, mut projection))) => {
            *preview_transform = *camera_transform;
            if let Projection::Perspective(ref mut persp) = *projection {
                persp.fov = camera_data.fov.to_radians();
            }
        }
        // Camera selected but no preview camera - spawn one
        (Some((camera_transform, camera_data)), None) => {
            commands.spawn((
                Camera3d::default(),
                Camera {
                    target: camera_preview_image.0.clone().into(),
                    clear_color: ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12)),
                    order: -1, // Render before main camera
                    ..default()
                },
                Projection::Perspective(PerspectiveProjection {
                    fov: camera_data.fov.to_radians(),
                    aspect_ratio: PREVIEW_WIDTH as f32 / PREVIEW_HEIGHT as f32,
                    ..default()
                }),
                *camera_transform,
                CameraPreviewMarker,
                EditorOnly,
                // Only render scene layer 0, not gizmos layer 1
                preview_camera_layers(),
            ));
        }
        // No camera selected but preview camera exists - despawn it
        (None, Some((entity, _, _))) => {
            commands.entity(entity).despawn();
        }
        // No camera selected and no preview camera - nothing to do
        (None, None) => {}
    }
}
