//! Camera preview — renders from a scene camera's viewpoint into a
//! small offscreen texture, displayed in the camera preview panel.
//!
//! Priority: selected Camera3d > DefaultCamera > first scene Camera3d.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::RenderTarget;
use bevy::core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass, NormalPrepass};
use bevy::core_pipeline::Skybox;
use bevy::light::{EnvironmentMapLight, GeneratedEnvironmentMapLight};
use bevy::prelude::*;
use bevy::render::view::Hdr;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};

use renzora::core::{
    DefaultCamera, EditorCamera, EditorLocked, HideInHierarchy, IsolatedCamera,
    PrimaryViewportCamera,
};
use renzora_editor::{DockingState, EditorSelection};

/// Skybox brightness for the shared atmosphere cubemap when synthesizing a
/// `Skybox` from the primary's `GeneratedEnvironmentMapLight`. Matches
/// `renzora_engine::camera::SHARED_SKY_BRIGHTNESS` so the preview's sky reads
/// the same as the secondary viewports'.
const SHARED_SKY_BRIGHTNESS: f32 = 1.0;

/// Initial preview image size. The render target is resized every frame to
/// match the panel's pixel rect (see `PreviewResizeRequest`), so this is just
/// the size used until the panel first reports its dimensions.
const PREVIEW_WIDTH: u32 = 640;
const PREVIEW_HEIGHT: u32 = 360;

/// Panel-driven resize request for the preview render texture. The panel's
/// `ui()` writes the desired pixel size each frame (logical size × DPI), and
/// `resize_camera_preview` applies it to the render image so the preview
/// renders at native resolution instead of being upscaled from a fixed
/// 640×360 (which looked blurry on larger panels). Mirrors the per-slot
/// `SlotResizeRequest` used by the main viewports.
#[derive(Resource)]
pub struct PreviewResizeRequest {
    pub width: std::sync::atomic::AtomicU32,
    pub height: std::sync::atomic::AtomicU32,
}

impl Default for PreviewResizeRequest {
    fn default() -> Self {
        Self {
            width: std::sync::atomic::AtomicU32::new(PREVIEW_WIDTH),
            height: std::sync::atomic::AtomicU32::new(PREVIEW_HEIGHT),
        }
    }
}

/// Resource holding the camera preview render texture.
#[derive(Resource)]
pub struct CameraPreviewState {
    pub image_handle: Handle<Image>,
    /// The entity whose camera we're currently previewing.
    pub previewing: Option<Entity>,
    /// Render-target pixel size last applied. Used to skip redundant resizes.
    pub current_size: UVec2,
}

/// Marker component for the preview camera entity.
#[derive(Component)]
pub struct CameraPreviewMarker;

/// Creates the camera preview render texture.
pub fn setup_camera_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
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

    commands.insert_resource(CameraPreviewState {
        image_handle,
        previewing: None,
        current_size: UVec2::new(PREVIEW_WIDTH, PREVIEW_HEIGHT),
    });
    commands.init_resource::<PreviewResizeRequest>();
}

/// Applies the panel's requested pixel size to the preview render texture so it
/// renders at native resolution (no upscaling blur). Runs each frame; cheap
/// no-op when the size is unchanged.
pub fn resize_camera_preview(
    req: Res<PreviewResizeRequest>,
    mut state: ResMut<CameraPreviewState>,
    mut images: ResMut<Assets<Image>>,
) {
    use std::sync::atomic::Ordering;
    let w = req.width.load(Ordering::Relaxed).clamp(64, 7680);
    let h = req.height.load(Ordering::Relaxed).clamp(64, 4320);
    let requested = UVec2::new(w, h);
    if state.current_size == requested {
        return;
    }
    if let Some(image) = images.get_mut(&state.image_handle) {
        image.resize(Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        });
        state.current_size = requested;
    }
}

/// Manages the camera preview — spawns/updates/despawns the preview camera.
///
/// Shows: selected Camera3d → DefaultCamera → first scene Camera3d.
/// Run condition: true when the Camera Preview panel is in the active egui dock
/// tree, or the bevy_ui native preview panel is mounted (its image exists).
pub fn camera_preview_panel_mounted(
    docking: Option<Res<DockingState>>,
    native: Query<(), With<crate::native_camera_preview::NativeCamPreview>>,
) -> bool {
    docking.is_some_and(|d| d.tree.contains_panel("camera_preview")) || !native.is_empty()
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
    let mounted = docking.is_some_and(|d| d.tree.contains_panel("camera_preview"));
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
        (
            Entity,
            &GlobalTransform,
            &Projection,
            Option<&DefaultCamera>,
        ),
        (
            With<Camera3d>,
            Without<CameraPreviewMarker>,
            Without<EditorCamera>,
        ),
    >,
    mut preview_cameras: Query<
        (Entity, &mut Transform, &mut Projection),
        With<CameraPreviewMarker>,
    >,
    editor_cameras: Query<
        &Camera,
        (With<EditorCamera>, Without<CameraPreviewMarker>),
    >,
    // Sky + IBL source: the PRIMARY viewport camera holds the single baked
    // atmosphere. We read it directly (not via the focused EditorCamera) because
    // the primary renders sky through its own `Atmosphere` pass and carries NO
    // `Skybox` component — only the baked `GeneratedEnvironmentMapLight`. Reading
    // the focused EditorCamera meant the preview only saw a sky when a *secondary*
    // viewport (which DOES get a synthesized `Skybox`) was focused.
    primary_env: Query<
        (
            Option<&Skybox>,
            Option<&GeneratedEnvironmentMapLight>,
            Option<&EnvironmentMapLight>,
        ),
        (With<PrimaryViewportCamera>, Without<CameraPreviewMarker>),
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
            scene_cameras
                .iter()
                .find(|(_, _, _, dc)| dc.is_some())
                .map(|(e, gt, p, _)| (e, gt, p))
        })
        .or_else(|| scene_cameras.iter().next().map(|(e, gt, p, _)| (e, gt, p)));

    let existing_preview = preview_cameras.iter_mut().next();

    let editor_clear_color = editor_cameras
        .iter()
        .next()
        .map(|cam| cam.clear_color)
        .unwrap_or(ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12)));

    // Mirror the primary viewport camera's sky + IBL onto the preview, so it
    // shows the same atmosphere as the viewports regardless of focus.
    let (preview_skybox, preview_env): (Option<Skybox>, Option<EnvironmentMapLight>) = primary_env
        .single()
        .ok()
        .map(|(sb, generated, env)| {
            // Prefer an explicit `Skybox` if the primary has one; otherwise
            // synthesize one from the baked atmosphere cubemap.
            let skybox = sb.cloned().or_else(|| {
                generated.map(|g| Skybox {
                    image: g.environment_map.clone(),
                    brightness: SHARED_SKY_BRIGHTNESS,
                    rotation: Quat::IDENTITY,
                })
            });
            (skybox, env.cloned())
        })
        .unwrap_or((None, None));

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
                // Sync sky + IBL every frame so the preview tracks the
                // primary's atmosphere bake.
                if let Some(ref skybox) = preview_skybox {
                    commands.entity(entity).try_insert(skybox.clone());
                } else {
                    commands.entity(entity).remove::<Skybox>();
                }
                if let Some(ref env) = preview_env {
                    commands.entity(entity).try_insert(env.clone());
                }
            }
            None => {
                let mut ecmds = commands.spawn((
                    Camera3d::default(),
                    // Mirror the viewport cameras' render config so this preview
                    // shares the ONE pbr + prepass pipeline format engine-wide
                    // (grouped into a sub-tuple to stay under the bundle-tuple
                    // limit). `DeferredPrepass` is added by
                    // `ensure_deferred_prepass_on_cameras` in deferred mode.
                    (Hdr, NormalPrepass, DepthPrepass, MotionVectorPrepass),
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
                if let Some(skybox) = preview_skybox {
                    ecmds.insert(skybox);
                }
                if let Some(env) = preview_env {
                    ecmds.insert(env);
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
