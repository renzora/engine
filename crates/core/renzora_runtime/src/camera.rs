//! Runtime camera spawning and render target syncing.

use bevy::prelude::*;
use bevy::camera::RenderTarget;

use crate::{EditorCamera, EditorLocked, HideInHierarchy, ViewportRenderTarget};

/// Spawns the editor's 3D scene-navigation camera.
///
/// If `ViewportRenderTarget` already has an image (editor mode),
/// the camera renders to it. Otherwise it renders to the window.
/// The camera is hidden from the hierarchy and locked from editing.
pub fn spawn_editor_camera(mut commands: Commands, render_target: Res<ViewportRenderTarget>) {
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
        Name::new("Editor Camera"),
    ));

    if let Some(ref image) = render_target.image {
        entity.insert(RenderTarget::Image(image.clone().into()));
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
        for entity in &cameras {
            commands
                .entity(entity)
                .insert(RenderTarget::Image(image.clone().into()));
        }
    }
}
