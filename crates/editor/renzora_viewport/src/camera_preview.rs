//! Camera preview — renders from a scene camera's viewpoint into a
//! small offscreen texture, displayed in the camera preview panel.
//!
//! Priority: selected Camera3d > DefaultCamera > first scene Camera3d.

use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::Skybox;
use bevy::ecs::reflect::ReflectComponent;
use bevy::ecs::world::FilteredEntityRef;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy_egui::{EguiTextureHandle, EguiUserTextures};

use renzora_editor::EditorSelection;
use renzora_runtime::{DefaultCamera, EditorCamera, EditorLocked, HideInHierarchy};

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

/// Sync post-processing components from the source scene camera to the preview camera.
pub fn sync_preview_post_processing(world: &mut World) {
    let Some(preview_state) = world.get_resource::<CameraPreviewState>() else { return };
    let Some(src) = preview_state.previewing else { return };

    // Find the preview camera entity
    let mut preview_entity = None;
    let mut q = world.query_filtered::<Entity, With<CameraPreviewMarker>>();
    for e in q.iter(world) {
        preview_entity = Some(e);
        break;
    }
    let Some(dst) = preview_entity else { return };
    if src == dst { return; }

    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let registry = type_registry.read();

    // Collect components ending in "Settings" from the source camera
    let mut to_sync: Vec<(ReflectComponent, Box<dyn Reflect>)> = Vec::new();
    let mut synced_paths: Vec<&'static str> = Vec::new();

    let entity_ref = world.entity(src);
    for reg in registry.iter() {
        let Some(rc) = reg.data::<ReflectComponent>() else { continue };
        let tp = reg.type_info().type_path();
        if !tp.ends_with("Settings") { continue; }
        if let Some(reflected) = rc.reflect(FilteredEntityRef::from(entity_ref)) {
            if let Ok(cloned) = reflected.reflect_clone() {
                to_sync.push((rc.clone(), cloned));
                synced_paths.push(tp);
            }
        }
    }
    drop(registry);

    // Apply to preview camera
    {
        let registry = type_registry.read();
        for (rc, value) in &to_sync {
            let mut entity_mut = world.entity_mut(dst);
            if rc.contains(entity_mut.as_readonly()) {
                rc.apply(entity_mut, value.as_partial_reflect());
            } else {
                rc.insert(&mut entity_mut, value.as_partial_reflect(), &registry);
            }
        }
    }

    // Remove stale components
    let registry = type_registry.read();
    let mut to_remove: Vec<ReflectComponent> = Vec::new();
    let editor_ref = world.entity(dst);
    for reg in registry.iter() {
        let Some(rc) = reg.data::<ReflectComponent>() else { continue };
        let tp = reg.type_info().type_path();
        if !tp.ends_with("Settings") { continue; }
        if rc.contains(FilteredEntityRef::from(editor_ref)) && !synced_paths.contains(&tp) {
            to_remove.push(rc.clone());
        }
    }
    drop(registry);

    for rc in &to_remove {
        rc.remove(&mut world.entity_mut(dst));
    }
}
