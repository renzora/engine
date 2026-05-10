//! Runtime camera spawning and render target syncing.

use crate::{EditorCamera, EditorCamera2d, EditorLocked, HideInHierarchy, ViewportRenderTarget};
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Camera, RenderTarget};
use bevy::core_pipeline::prepass::{DepthPrepass, NormalPrepass};
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::light::AtmosphereEnvironmentMapLight;
use bevy::pbr::{Atmosphere, AtmosphereSettings, ScatteringMedium};
use bevy::prelude::*;
use bevy::render::view::Hdr;
use renzora::core::viewport_types::{ViewportSettings, ViewportState, ViewportView};
use renzora::core::PlayModeState;
use renzora::viewport_types::EditorCameraMatrix;

/// Spawns the editor's 3D scene-navigation camera.
///
/// If `ViewportRenderTarget` already has an image (editor mode),
/// the camera renders to it. Otherwise it renders to the window.
/// The camera is hidden from the hierarchy and locked from editing.
///
/// Render-effect components (`Atmosphere`, `AtmosphereSettings`,
/// `AtmosphereEnvironmentMapLight`, `Msaa::Off`, etc.) are attached at
/// spawn so Bevy 0.18's atmosphere/sky pipeline can lock in its bind
/// group layout once and never need to grow it. Trying to add atmosphere
/// at runtime crashes wgpu with "20 vs 23 bindings" — Bevy specializes
/// the layout per-camera at first render, and atmosphere bindings are
/// gated on whether the component existed at that moment.
///
/// `EffectRouting` + `renzora_atmosphere::sync_atmosphere` then *update*
/// these components in-place from a `WorldEnvironment` source entity (or
/// any entity with `AtmosphereComponentSettings`), giving us one logical
/// source of truth that drives both editor and play cameras identically.
/// The plugin replaces, never removes — see its file for the why.
pub fn spawn_editor_camera(
    mut commands: Commands,
    render_target: Res<ViewportRenderTarget>,
    mut mediums: ResMut<Assets<ScatteringMedium>>,
) {
    let default_medium = mediums.add(ScatteringMedium::default());

    let mut entity = commands.spawn((
        Camera3d::default(),
        Camera {
            order: -1,
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            far: 100_000.0,
            ..default()
        }),
        Transform::from_xyz(5.0, 4.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        EditorCamera,
        HideInHierarchy,
        EditorLocked,
        RenderLayers::from_layers(&[0, 1]),
        Name::new("Editor Camera"),
        Hdr,
        Atmosphere {
            bottom_radius: 6_360_000.0,
            top_radius: 6_460_000.0,
            ground_albedo: Vec3::splat(0.3),
            medium: default_medium,
        },
        AtmosphereSettings::default(),
        // IBL is off by default — `intensity: 0.0` keeps the bind-group
        // slots present (Bevy 0.18 won't let us add this component at
        // runtime without a pipeline crash) but contributes nothing
        // visually. Adding an `EnvironmentMapComponentSettings` to any
        // entity in the scene routes a non-zero intensity onto the
        // camera; removing it pushes intensity back to 0. See
        // `renzora_environment_map`.
        AtmosphereEnvironmentMapLight {
            intensity: 0.0,
            ..default()
        },
        // Atmosphere/sky binds depth as non-multisampled (binding 13);
        // any MSAA on the same camera trips a wgpu validation crash.
        Msaa::Off,
        // Force the prepass to carry world normals + depth. NormalPrepass
        // is required for `pbr_fragment.wgsl::pbr_input_from_vertex_output`
        // to compile against `alpha_mode = Mask` materials (the prepass
        // gates `world_normal` behind `NORMAL_PREPASS_OR_DEFERRED_PREPASS`).
        // DepthPrepass is required for SSGI / Lumen `ScreenSpace`. Bevy 0.18
        // specializes the prepass pipeline once on first render; both must
        // be present at spawn — adding either at runtime trips a wgpu
        // validation crash on the prepass attachment list.
        (NormalPrepass, DepthPrepass),
    ));

    if let Some(ref image) = render_target.image {
        info!("[camera] Editor camera spawned with offscreen render target");
        entity.insert(RenderTarget::Image(image.clone().into()));
    } else {
        info!("[camera] Editor camera spawned rendering to window (no viewport image yet)");
    }
}

/// Spawns the editor's 2D scene-navigation camera.
///
/// Sibling of [`spawn_editor_camera`] — orthographic camera that renders
/// to the same viewport offscreen image. Starts inactive; the
/// `sync_viewport_camera_activation` system in `renzora_viewport` toggles
/// it active when `ViewportSettings.viewport_view == ViewportView::Two`.
///
/// Only one of the two editor cameras is ever active at a time, so they
/// can safely share the render target.
pub fn spawn_editor_2d_camera(mut commands: Commands, render_target: Res<ViewportRenderTarget>) {
    let mut entity = commands.spawn((
        Camera2d,
        Camera {
            // Match the 3D editor camera's order so cycling between views
            // doesn't change z-stacking against any other cameras (e.g. UI).
            order: -1,
            // Inactive until the user picks the 2D viewport view; otherwise
            // both editor cameras would race for the offscreen target.
            is_active: false,
            ..default()
        },
        Transform::default(),
        EditorCamera2d,
        HideInHierarchy,
        EditorLocked,
        Name::new("Editor Camera 2D"),
    ));

    if let Some(ref image) = render_target.image {
        entity.insert(RenderTarget::Image(image.clone().into()));
    }
}

/// Tracks the last selection processed for auto-view-switching, so the
/// 2D-flip fires on selection *change* only — same pattern the UI
/// auto-switch uses, but kept independent so the two systems don't
/// fight over a shared tracker.
#[cfg(feature = "editor")]
#[derive(Resource, Default)]
pub struct LastSelectionForView2dSwitch(pub Option<bevy::ecs::entity::Entity>);

/// When the selection changes to a 2D entity (Sprite or Camera2d), flip the
/// viewport to 2D view. When it changes to a non-2D entity *and* we're
/// currently in 2D view, fall back to 3D. Other view transitions (3D ↔ UI)
/// are left to the UI auto-switch system or the user.
#[cfg(feature = "editor")]
pub fn auto_switch_view_on_2d_selection(world: &mut World) {
    use renzora::core::viewport_types::{ViewportSettings, ViewportView};

    let current_sel = world
        .get_resource::<renzora_editor::EditorSelection>()
        .and_then(|s| s.get());
    let last_sel = world
        .get_resource::<LastSelectionForView2dSwitch>()
        .map(|l| l.0)
        .unwrap_or(None);
    if current_sel == last_sel {
        return;
    }
    if let Some(mut last) = world.get_resource_mut::<LastSelectionForView2dSwitch>() {
        last.0 = current_sel;
    }
    let Some(entity) = current_sel else { return };

    let is_2d = world.get::<bevy::sprite::Sprite>(entity).is_some()
        || world.get::<Camera2d>(entity).is_some()
        || world.get::<renzora::core::Node2d>(entity).is_some();

    let view = world
        .get_resource::<ViewportSettings>()
        .map(|s| s.viewport_view)
        .unwrap_or_default();
    let target = match (is_2d, view) {
        (true, ViewportView::Two) => return,
        (true, _) => ViewportView::Two,
        (false, ViewportView::Two) => ViewportView::Three,
        (false, _) => return,
    };
    if let Some(mut settings) = world.get_resource_mut::<ViewportSettings>() {
        settings.viewport_view = target;
    }
}

/// Pan + zoom controls for the editor 2D camera.
///
/// Only acts when `viewport_view == Two`, the cursor is over the viewport
/// panel, and we're in editing mode. Middle-mouse drag pans (screen pixels
/// converted to world units via the orthographic scale so drag stays
/// 1:1 with the cursor at any zoom). Scroll wheel adjusts ortho scale,
/// translating the camera so the world point under the cursor stays
/// pinned to the cursor — Photoshop-style zoom-toward-cursor.
pub fn editor_2d_camera_controller(
    settings: Option<Res<ViewportSettings>>,
    viewport: Option<Res<ViewportState>>,
    play_mode: Option<Res<PlayModeState>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut scroll_events: MessageReader<MouseWheel>,
    windows: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    mut camera_query: Query<(&Camera, &mut Transform, &mut Projection), With<EditorCamera2d>>,
) {
    let in_play = play_mode.map_or(false, |pm| pm.is_in_play_mode());
    let view = settings.map(|s| s.viewport_view).unwrap_or_default();
    if in_play || view != ViewportView::Two {
        mouse_motion.clear();
        scroll_events.clear();
        return;
    }

    let hovered = viewport.as_ref().map_or(false, |v| v.hovered);
    let Ok((camera, mut transform, mut projection)) = camera_query.single_mut() else {
        mouse_motion.clear();
        scroll_events.clear();
        return;
    };

    // Pan: middle-mouse drag converts screen pixels to world units via scale.
    if hovered && mouse_button.pressed(MouseButton::Middle) {
        let mut delta = Vec2::ZERO;
        for ev in mouse_motion.read() {
            delta += ev.delta;
        }
        if delta != Vec2::ZERO {
            let scale = match &*projection {
                Projection::Orthographic(o) => o.scale,
                _ => 1.0,
            };
            transform.translation.x -= delta.x * scale;
            // Screen y increases downward, world y increases upward.
            transform.translation.y += delta.y * scale;
        }
    } else {
        mouse_motion.clear();
    }

    // Zoom: scroll wheel. Each notch is 10% in/out, clamped to a
    // generous range so the camera doesn't disappear from extreme edits.
    if hovered {
        let mut zoom = 0.0_f32;
        for ev in scroll_events.read() {
            zoom += ev.y;
        }
        if zoom != 0.0 {
            if let Projection::Orthographic(ref mut o) = *projection {
                // Capture cursor world position BEFORE scale change so we
                // can adjust the camera translation to keep that world
                // point under the cursor.
                let cursor_world_before = viewport.as_deref().and_then(|vs| {
                    let window = windows.single().ok()?;
                    let cursor = window.cursor_position()?;
                    let in_rect = cursor - vs.screen_position;
                    if in_rect.x < 0.0
                        || in_rect.y < 0.0
                        || in_rect.x >= vs.screen_size.x
                        || in_rect.y >= vs.screen_size.y
                    {
                        return None;
                    }
                    let image_size = vs.current_size.as_vec2();
                    if image_size.x <= 0.0 || image_size.y <= 0.0 {
                        return None;
                    }
                    let scaled = Vec2::new(
                        in_rect.x * image_size.x / vs.screen_size.x,
                        in_rect.y * image_size.y / vs.screen_size.y,
                    );
                    let cam_gt = GlobalTransform::from(*transform);
                    camera.viewport_to_world_2d(&cam_gt, scaled).ok()
                });

                let old_scale = o.scale;
                let step: f32 = 0.9;
                o.scale = (o.scale * step.powf(zoom)).clamp(0.01, 1000.0);
                let new_scale = o.scale;

                // Translate so the captured world point stays pinned to
                // the cursor. The new world point at the same cursor
                // image-pixel scales linearly with the projection scale,
                // so `delta_cam = (cursor_world - cam) * (1 - new/old)`.
                if let Some(cursor_world) = cursor_world_before {
                    if old_scale > 0.0 {
                        let cam_xy = transform.translation.truncate();
                        let offset = cursor_world - cam_xy;
                        let zoom_factor = new_scale / old_scale;
                        transform.translation.x += offset.x * (1.0 - zoom_factor);
                        transform.translation.y += offset.y * (1.0 - zoom_factor);
                    }
                }
            }
        }
    } else {
        scroll_events.clear();
    }
}

/// Observer: every time a `Camera2d` is inserted (preset spawn, scene
/// reflection load, runtime spawn), shift the projection's
/// `viewport_origin` so the camera's transform position maps to the
/// **top-left** of the rendered viewport.
///
/// This keeps Bevy's 2D world consistent with our Godot-style
/// convention: the project's window-area outline is anchored at world
/// (0, 0) extending to (width, -height), and a Camera 2D at world
/// origin renders that area exactly. Default Bevy `viewport_origin`
/// is `(0.5, 0.5)` (centred), which would render world (0, 0) at the
/// middle of the runtime window — sprites authored against the
/// outline would appear off-centre.
pub fn on_camera_2d_inserted(
    trigger: On<Insert, Camera2d>,
    mut projections: Query<&mut Projection>,
) {
    let entity = trigger.entity;
    if let Ok(mut projection) = projections.get_mut(entity) {
        if let Projection::Orthographic(ortho) = projection.as_mut() {
            ortho.viewport_origin = Vec2::new(0.0, 1.0);
        }
    }
}

/// Companion observer: when `Projection` itself is inserted on an
/// entity that already carries `Camera2d`, re-apply the top-left
/// `viewport_origin`. Reflection-based scene loads insert `Camera2d`
/// first (which fires `on_camera_2d_inserted` and fixes the projection)
/// *and then* deserialize the saved `Projection` on top — overwriting
/// the fix with whatever was on disk. This catches that second insert
/// so legacy scenes converge to the Godot-style convention as soon as
/// they load. No-op for `Camera3d` and any other camera kind.
pub fn on_projection_inserted_for_2d(
    trigger: On<Insert, Projection>,
    cameras_2d: Query<(), With<Camera2d>>,
    mut projections: Query<&mut Projection>,
) {
    let entity = trigger.entity;
    if cameras_2d.get(entity).is_err() {
        return;
    }
    if let Ok(mut projection) = projections.get_mut(entity) {
        if let Projection::Orthographic(ortho) = projection.as_mut() {
            ortho.viewport_origin = Vec2::new(0.0, 1.0);
        }
    }
}

/// Watches for changes to `ViewportRenderTarget` and updates both editor
/// cameras accordingly.
///
/// Only acts when an image handle is set (editor mode). When `None`, the cameras
/// keep their default window target — we never remove `RenderTarget`.
pub fn sync_camera_render_target(
    render_target: Res<ViewportRenderTarget>,
    cameras_3d: Query<Entity, With<EditorCamera>>,
    cameras_2d: Query<Entity, With<EditorCamera2d>>,
    mut commands: Commands,
) {
    if !render_target.is_changed() {
        return;
    }

    if let Some(ref image) = render_target.image {
        info!(
            "[camera] ViewportRenderTarget changed — redirecting editor cameras to offscreen image"
        );
        for entity in cameras_3d.iter().chain(cameras_2d.iter()) {
            commands
                .entity(entity)
                .insert(RenderTarget::Image(image.clone().into()));
        }
    } else {
        info!("[camera] ViewportRenderTarget changed — but image is None");
    }
}

/// Cache the editor camera's clip-from-world matrix into a resource each frame,
/// so overlay systems (grid, gizmos) can CPU-project geometry without querying
/// the camera themselves (which requires mutable World access).
pub fn update_editor_camera_matrix(
    cameras: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    mut mat: ResMut<EditorCameraMatrix>,
) {
    let Ok((camera, transform)) = cameras.single() else {
        mat.valid = false;
        return;
    };
    let view_from_world = transform.affine().inverse();
    let clip_from_view = camera.clip_from_view();
    mat.clip_from_world = clip_from_view * Mat4::from(view_from_world);
    mat.world_from_clip = mat.clip_from_world.inverse();
    mat.cam_pos = transform.translation();
    mat.cam_forward = *transform.forward();
    mat.valid = true;
}
