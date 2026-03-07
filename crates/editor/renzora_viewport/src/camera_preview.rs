//! Camera preview — renders from a selected camera entity's viewpoint into a
//! small offscreen texture, displayed in the inspector.

use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::Skybox;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy_egui::{EguiTextureHandle, EguiUserTextures};

use renzora_editor::EditorSelection;
use renzora_runtime::{EditorCamera, EditorLocked, HideInHierarchy};

/// Preview image size (kept small for performance).
const PREVIEW_WIDTH: u32 = 640;
const PREVIEW_HEIGHT: u32 = 360;

/// Resource holding the camera preview render texture and egui texture id.
#[derive(Resource)]
pub struct CameraPreviewState {
    pub image_handle: Handle<Image>,
    pub texture_id: Option<bevy_egui::egui::TextureId>,
    /// The entity whose camera we're currently previewing.
    pub previewing: Option<Entity>,
}

/// Marker component for the preview camera entity.
#[derive(Component)]
pub struct CameraPreviewMarker;

/// Creates the camera preview render texture and registers it with egui.
pub fn setup_camera_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut user_textures: ResMut<EguiUserTextures>,
) {
    let size = Extent3d {
        width: PREVIEW_WIDTH,
        height: PREVIEW_HEIGHT,
        depth_or_array_layers: 1,
    };

    let mut image = Image {
        data: Some(vec![0u8; (size.width * size.height * 4) as usize]),
        ..default()
    };
    image.texture_descriptor.size = size;
    image.texture_descriptor.format = TextureFormat::Bgra8UnormSrgb;
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;

    let image_handle = images.add(image);

    user_textures.add_image(EguiTextureHandle::Strong(image_handle.clone()));

    let texture_id = user_textures.image_id(image_handle.id());

    commands.insert_resource(CameraPreviewState {
        image_handle,
        texture_id,
        previewing: None,
    });
}

/// Manages the camera preview — spawns/updates/despawns the preview camera
/// based on whether a Camera3d entity is selected.
pub fn update_camera_preview(
    mut commands: Commands,
    selection: Res<EditorSelection>,
    mut preview_state: ResMut<CameraPreviewState>,
    scene_cameras: Query<
        (&Transform, &Projection),
        (With<Camera3d>, Without<CameraPreviewMarker>, Without<EditorCamera>),
    >,
    mut preview_cameras: Query<
        (Entity, &mut Transform, &mut Projection),
        With<CameraPreviewMarker>,
    >,
    editor_cameras: Query<
        (Option<&Skybox>, &Camera),
        (With<EditorCamera>, Without<CameraPreviewMarker>),
    >,
) {
    let selected = selection.get();

    // Check if the selected entity has a Camera3d (and isn't the editor camera)
    let selected_camera = selected.and_then(|e| scene_cameras.get(e).ok());

    let existing_preview = preview_cameras.iter_mut().next();

    // Get the editor camera's skybox and clear color so the preview matches
    let (editor_skybox, editor_clear_color) = editor_cameras
        .iter()
        .next()
        .map(|(skybox, cam)| (skybox, cam.clear_color.clone()))
        .unwrap_or((None, ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12))));

    if let Some((cam_transform, cam_projection)) = selected_camera {
        let selected_entity = selected.unwrap();
        preview_state.previewing = Some(selected_entity);

        match existing_preview {
            Some((_entity, mut preview_transform, mut preview_proj)) => {
                *preview_transform = *cam_transform;
                *preview_proj = cam_projection.clone();
            }
            None => {
                let mut ecmds = commands.spawn((
                    Camera3d::default(),
                    Msaa::Off,
                    Camera {
                        clear_color: editor_clear_color,
                        order: -2,
                        ..default()
                    },
                    RenderTarget::Image(preview_state.image_handle.clone().into()),
                    cam_projection.clone(),
                    *cam_transform,
                    CameraPreviewMarker,
                    HideInHierarchy,
                    EditorLocked,
                    // Scene only (layer 0) — no gizmos (layer 1)
                    RenderLayers::layer(0),
                    Name::new("Camera Preview"),
                ));
                // Copy skybox from editor camera
                if let Some(skybox) = editor_skybox {
                    ecmds.insert(skybox.clone());
                }
            }
        }
    } else {
        // No camera selected — despawn preview camera if it exists
        preview_state.previewing = None;
        if let Some((entity, _, _)) = existing_preview {
            commands.entity(entity).despawn();
        }
    }
}
