//! Camera preview — renders from a scene camera's viewpoint into a
//! small offscreen texture, displayed in the camera preview panel.
//!
//! Priority: selected Camera3d > DefaultCamera > first scene Camera3d.

use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::Skybox;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy_egui::{EguiTextureHandle, EguiUserTextures};

use renzora_editor::{DockingState, EditorSelection};
use renzora::core::{DefaultCamera, EditorCamera, EditorLocked, HideInHierarchy, IsolatedCamera};

/// Preview image size.
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

/// Manages the camera preview — spawns/updates/despawns the preview camera.
///
/// Shows: selected Camera3d → DefaultCamera → first scene Camera3d.
/// Run condition: true when the Camera Preview panel is in the active dock tree.
pub fn camera_preview_panel_mounted(docking: Option<Res<DockingState>>) -> bool {
    docking.map_or(false, |d| d.tree.contains_panel("camera_preview"))
}

/// Toggles the Camera Preview camera on/off with panel visibility and despawns
/// the preview camera entity when the panel closes.
pub fn sync_camera_preview_activation(
    docking: Option<Res<DockingState>>,
    mut preview_cameras: Query<&mut Camera, With<CameraPreviewMarker>>,
    preview_entities: Query<Entity, With<CameraPreviewMarker>>,
    mut preview_state: Option<ResMut<CameraPreviewState>>,
    mut commands: Commands,
) {
    let mounted = docking.map_or(false, |d| d.tree.contains_panel("camera_preview"));
    for mut camera in preview_cameras.iter_mut() {
        if camera.is_active != mounted {
            camera.is_active = mounted;
        }
    }
    if !mounted {
        for entity in preview_entities.iter() {
            commands.entity(entity).despawn();
        }
        if let Some(ref mut state) = preview_state {
            state.previewing = None;
        }
    }
}

pub fn update_camera_preview(
    mut commands: Commands,
    selection: Res<EditorSelection>,
    mut preview_state: ResMut<CameraPreviewState>,
    scene_cameras: Query<
        (Entity, &GlobalTransform, &Projection, Option<&DefaultCamera>),
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

    // Pick which camera to preview:
    // 1. Selected entity if it has Camera3d
    // 2. Entity with DefaultCamera marker
    // 3. First scene camera
    let target = selected
        .and_then(|e| scene_cameras.get(e).ok())
        .map(|(e, gt, p, _)| (e, gt, p))
        .or_else(|| {
            scene_cameras.iter()
                .find(|(_, _, _, dc)| dc.is_some())
                .map(|(e, gt, p, _)| (e, gt, p))
        })
        .or_else(|| {
            scene_cameras.iter()
                .next()
                .map(|(e, gt, p, _)| (e, gt, p))
        });

    let existing_preview = preview_cameras.iter_mut().next();

    let (editor_skybox, editor_clear_color) = editor_cameras
        .iter()
        .next()
        .map(|(skybox, cam)| (skybox.cloned(), cam.clear_color.clone()))
        .unwrap_or((None, ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12))));

    if let Some((cam_entity, cam_global_transform, cam_projection)) = target {
        preview_state.previewing = Some(cam_entity);

        // Decompose GlobalTransform into a local Transform for the preview camera
        let (scale, rotation, translation) = cam_global_transform.to_scale_rotation_translation();
        let cam_transform = Transform {
            translation,
            rotation,
            scale,
        };

        match existing_preview {
            Some((entity, mut preview_transform, mut preview_proj)) => {
                *preview_transform = cam_transform;
                *preview_proj = cam_projection.clone();
                // Sync skybox every frame
                if let Some(ref skybox) = editor_skybox {
                    commands.entity(entity).insert(skybox.clone());
                } else {
                    commands.entity(entity).remove::<Skybox>();
                }
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
                    cam_transform,
                    CameraPreviewMarker,
                    IsolatedCamera,
                    HideInHierarchy,
                    EditorLocked,
                    RenderLayers::layer(0),
                    Name::new("Camera Preview"),
                ));
                if let Some(skybox) = editor_skybox {
                    ecmds.insert(skybox);
                }
            }
        }
    } else {
        preview_state.previewing = None;
        if let Some((entity, _, _)) = existing_preview {
            commands.entity(entity).despawn();
        }
    }
}
