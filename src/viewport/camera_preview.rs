//! Camera preview system - renders from selected camera's viewpoint

use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};

use crate::core::SelectionState;
use crate::gizmo::preview_camera_layers;
use crate::shared::CameraNodeData;
use crate::scene::EditorOnly;
use crate::shared::CameraRigData;

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
/// based on whether a Camera3D or CameraRig node is selected
pub fn update_camera_preview(
    mut commands: Commands,
    selection: Res<SelectionState>,
    camera_preview_image: Res<CameraPreviewImage>,
    camera_nodes: Query<(&Transform, &CameraNodeData), (Without<CameraPreviewMarker>, Without<CameraRigData>)>,
    camera_rigs: Query<(&Transform, &CameraRigData), (Without<CameraPreviewMarker>, Without<CameraNodeData>)>,
    mut preview_camera: Query<
        (Entity, &mut Transform, &mut Projection),
        With<CameraPreviewMarker>,
    >,
) {
    // Check if a camera node or camera rig is selected
    let selected_camera = selection
        .selected_entity
        .and_then(|entity| camera_nodes.get(entity).ok());

    let selected_rig = selection
        .selected_entity
        .and_then(|entity| camera_rigs.get(entity).ok());

    let existing_preview = preview_camera.iter_mut().next();

    // Handle Camera3D nodes
    if let Some((camera_transform, camera_data)) = selected_camera {
        match existing_preview {
            Some((_, mut preview_transform, mut projection)) => {
                *preview_transform = *camera_transform;
                if let Projection::Perspective(ref mut persp) = *projection {
                    persp.fov = camera_data.fov.to_radians();
                }
            }
            None => {
                commands.spawn((
                    Camera3d::default(),
                    Msaa::Off,
                    Camera {
                        clear_color: ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12)),
                        order: -1,
                        ..default()
                    },
                    RenderTarget::Image(camera_preview_image.0.clone().into()),
                    Projection::Perspective(PerspectiveProjection {
                        fov: camera_data.fov.to_radians(),
                        aspect_ratio: PREVIEW_WIDTH as f32 / PREVIEW_HEIGHT as f32,
                        ..default()
                    }),
                    *camera_transform,
                    CameraPreviewMarker,
                    EditorOnly,
                    preview_camera_layers(),
                ));
            }
        }
    }
    // Handle CameraRig nodes
    else if let Some((rig_transform, rig_data)) = selected_rig {
        match existing_preview {
            Some((_, mut preview_transform, mut projection)) => {
                *preview_transform = *rig_transform;
                if let Projection::Perspective(ref mut persp) = *projection {
                    persp.fov = rig_data.fov.to_radians();
                }
            }
            None => {
                commands.spawn((
                    Camera3d::default(),
                    Msaa::Off,
                    Camera {
                        clear_color: ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12)),
                        order: -1,
                        ..default()
                    },
                    RenderTarget::Image(camera_preview_image.0.clone().into()),
                    Projection::Perspective(PerspectiveProjection {
                        fov: rig_data.fov.to_radians(),
                        aspect_ratio: PREVIEW_WIDTH as f32 / PREVIEW_HEIGHT as f32,
                        ..default()
                    }),
                    *rig_transform,
                    CameraPreviewMarker,
                    EditorOnly,
                    preview_camera_layers(),
                ));
            }
        }
    }
    // No camera selected but preview camera exists - despawn it
    else if let Some((entity, _, _)) = existing_preview {
        commands.entity(entity).despawn();
    }
}
