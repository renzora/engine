//! Runtime camera spawning and render target syncing.

use bevy::prelude::*;
use bevy::camera::{Camera, RenderTarget};
use bevy::camera::visibility::RenderLayers;
use bevy::render::view::Hdr;
use bevy::pbr::{Atmosphere, AtmosphereSettings, ScatteringMedium};
use renzora_core::viewport_types::EditorCameraMatrix;
use crate::{EditorCamera, EditorLocked, HideInHierarchy, ViewportRenderTarget};

/// Spawns the editor's 3D scene-navigation camera.
///
/// If `ViewportRenderTarget` already has an image (editor mode),
/// the camera renders to it. Otherwise it renders to the window.
/// The camera is hidden from the hierarchy and locked from editing.
pub fn spawn_editor_camera(
    mut commands: Commands,
    render_target: Res<ViewportRenderTarget>,
    mut mediums: ResMut<Assets<ScatteringMedium>>,
) {
    // Pre-insert Atmosphere so Bevy's mesh view bind group layout always
    // includes atmosphere bindings. Without this, adding atmosphere at
    // runtime causes a bind group mismatch crash (20 vs 23 bindings).
    let default_medium = mediums.add(ScatteringMedium::default());

    let mut entity = commands.spawn((
        Camera3d::default(),
        Camera {
            order: -1,
            ..default()
        },
        Transform::from_xyz(5.0, 4.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        EditorCamera,
        HideInHierarchy,
        EditorLocked,
        RenderLayers::from_layers(&[0, 1]),
        Name::new("Editor Camera"),
        Atmosphere {
            bottom_radius: 6_360_000.0,
            top_radius: 6_460_000.0,
            ground_albedo: Vec3::splat(0.3),
            medium: default_medium,
        },
        AtmosphereSettings::default(),
        Hdr,
    ));

    if let Some(ref image) = render_target.image {
        info!("[camera] Editor camera spawned with offscreen render target");
        entity.insert(RenderTarget::Image(image.clone().into()));
    } else {
        info!("[camera] Editor camera spawned rendering to window (no viewport image yet)");
    }
}

/// Watches for changes to `ViewportRenderTarget` and updates the camera accordingly.
///
/// Only acts when an image handle is set (editor mode). When `None`, the camera
/// keeps its default window target — we never remove `RenderTarget`.
pub fn sync_camera_render_target(
    render_target: Res<ViewportRenderTarget>,
    cameras: Query<Entity, With<EditorCamera>>,
    mut commands: Commands,
) {
    if !render_target.is_changed() {
        return;
    }

    if let Some(ref image) = render_target.image {
        info!("[camera] ViewportRenderTarget changed — redirecting editor camera to offscreen image");
        for entity in &cameras {
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
