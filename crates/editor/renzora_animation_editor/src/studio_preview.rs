//! Studio Preview — isolated 3D viewport for animation preview.
//!
//! Creates an offscreen render target with its own camera, light, and a cloned
//! copy of the selected entity's model. The animation system drives playback
//! on the real entity while this panel mirrors it visually.

use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy_egui::egui::TextureId;
use bevy_egui::{EguiTextureHandle, EguiUserTextures};
use renzora_core::{IsolatedCamera, MeshInstanceData};
use renzora_runtime::{EditorLocked, HideInHierarchy};

use crate::AnimationEditorState;

pub const STUDIO_PREVIEW_LAYER: usize = 10;

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct StudioPreviewImage {
    pub handle: Handle<Image>,
    pub texture_id: Option<TextureId>,
}

impl Default for StudioPreviewImage {
    fn default() -> Self {
        Self {
            handle: Handle::default(),
            texture_id: None,
        }
    }
}

#[derive(Resource)]
pub struct StudioPreviewOrbit {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: Vec3,
}

impl Default for StudioPreviewOrbit {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.3,
            distance: 3.0,
            target: Vec3::new(0.0, 1.0, 0.0),
        }
    }
}

/// Tracks which scene entity is currently cloned into the preview.
#[derive(Resource, Default)]
pub struct StudioPreviewTracker {
    pub source_entity: Option<Entity>,
}

// ---------------------------------------------------------------------------
// Marker components
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct StudioPreviewCamera;

#[derive(Component)]
pub struct StudioPreviewLight;

/// Root of the cloned model in the preview scene.
#[derive(Component)]
pub struct StudioPreviewModel;

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

pub fn setup_studio_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut user_textures: ResMut<EguiUserTextures>,
) {
    let size = Extent3d {
        width: 512,
        height: 512,
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

    commands.insert_resource(StudioPreviewImage {
        handle: image_handle.clone(),
        texture_id,
    });

    // Camera
    commands.spawn((
        Camera3d::default(),
        Msaa::Off,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgba(0.06, 0.06, 0.08, 1.0)),
            order: -5,
            is_active: true,
            ..default()
        },
        RenderTarget::Image(image_handle.into()),
        Transform::from_xyz(0.0, 1.5, 3.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
        RenderLayers::layer(STUDIO_PREVIEW_LAYER),
        StudioPreviewCamera,
        IsolatedCamera,
        HideInHierarchy,
        EditorLocked,
        Name::new("Studio Preview Camera"),
    ));

    // Key light
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.98, 0.95),
            illuminance: 8000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.7, 0.4, 0.0)),
        RenderLayers::layer(STUDIO_PREVIEW_LAYER),
        StudioPreviewLight,
        HideInHierarchy,
        EditorLocked,
        Name::new("Studio Preview Key Light"),
    ));

    // Fill light (softer, opposite side)
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.7, 0.8, 1.0),
            illuminance: 3000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.3, -0.8, 0.0)),
        RenderLayers::layer(STUDIO_PREVIEW_LAYER),
        StudioPreviewLight,
        HideInHierarchy,
        EditorLocked,
        Name::new("Studio Preview Fill Light"),
    ));
}

// ---------------------------------------------------------------------------
// Model sync — clone the selected entity's GLTF scene into the preview layer
// ---------------------------------------------------------------------------

pub fn sync_preview_model(
    mut commands: Commands,
    editor_state: Res<AnimationEditorState>,
    mut tracker: ResMut<StudioPreviewTracker>,
    asset_server: Res<AssetServer>,
    gltf_assets: Res<Assets<Gltf>>,
    mesh_query: Query<&MeshInstanceData>,
    existing_preview: Query<Entity, With<StudioPreviewModel>>,
) {
    let selected = editor_state.selected_entity;

    // If selection hasn't changed, nothing to do
    if tracker.source_entity == selected {
        return;
    }
    tracker.source_entity = selected;

    // Despawn old preview model
    for entity in existing_preview.iter() {
        commands.entity(entity).despawn();
    }

    // If nothing selected, done
    let Some(source) = selected else { return };

    // Get the model path from the selected entity
    let Ok(mesh_data) = mesh_query.get(source) else {
        return;
    };
    let Some(ref model_path) = mesh_data.model_path else {
        return;
    };

    // Load the GLTF and find its default scene
    let gltf_handle: Handle<Gltf> = asset_server.load(model_path);
    let Some(gltf) = gltf_assets.get(&gltf_handle) else {
        // Not loaded yet — clear tracker so we retry next frame
        tracker.source_entity = None;
        return;
    };

    let scene = gltf
        .default_scene
        .clone()
        .or_else(|| gltf.scenes.first().cloned());

    let Some(scene_handle) = scene else { return };

    // Spawn the preview model on the studio preview render layer
    let root = commands
        .spawn((
            Transform::default(),
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
            ViewVisibility::default(),
            RenderLayers::layer(STUDIO_PREVIEW_LAYER),
            StudioPreviewModel,
            HideInHierarchy,
            EditorLocked,
            Name::new("Studio Preview Model"),
        ))
        .id();

    commands.spawn((
        bevy::scene::SceneRoot(scene_handle),
        Transform::default(),
        Visibility::Visible,
        InheritedVisibility::VISIBLE,
        ViewVisibility::default(),
        ChildOf(root),
    ));

    info!("[studio_preview] Loaded model '{}' into preview", model_path);
}

/// Continuously propagate `RenderLayers` to all descendants of preview model
/// entities. GLTF scenes spawn children asynchronously over multiple frames
/// and those children get default `RenderLayers` (layer 0), so we must
/// overwrite them with the preview layer.
pub fn propagate_preview_layer(
    mut commands: Commands,
    preview_roots: Query<Entity, With<StudioPreviewModel>>,
    children_query: Query<&Children>,
    layer_query: Query<&RenderLayers>,
) {
    let target = RenderLayers::layer(STUDIO_PREVIEW_LAYER);

    for root in preview_roots.iter() {
        let mut stack: Vec<Entity> = Vec::new();
        if let Ok(children) = children_query.get(root) {
            stack.extend(children.iter());
        }

        while let Some(child) = stack.pop() {
            // Overwrite if missing or set to a different layer
            let needs_update = match layer_query.get(child) {
                Ok(layers) => *layers != target,
                Err(_) => true,
            };
            if needs_update {
                commands.entity(child).insert(target.clone());
            }

            if let Ok(grandchildren) = children_query.get(child) {
                stack.extend(grandchildren.iter());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Orbit camera
// ---------------------------------------------------------------------------

pub fn update_studio_preview_camera(
    orbit: Res<StudioPreviewOrbit>,
    mut camera: Query<&mut Transform, With<StudioPreviewCamera>>,
) {
    for mut transform in camera.iter_mut() {
        let x = orbit.distance * orbit.pitch.cos() * orbit.yaw.sin();
        let y = orbit.distance * orbit.pitch.sin();
        let z = orbit.distance * orbit.pitch.cos() * orbit.yaw.cos();

        transform.translation = orbit.target + Vec3::new(x, y, z);
        transform.look_at(orbit.target, Vec3::Y);
    }
}
