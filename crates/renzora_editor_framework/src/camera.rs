//! Editor UI camera setup

use bevy::prelude::*;

/// Marker for the editor's UI camera so play mode can disable it.
#[derive(Component)]
pub struct EditorUiCamera;

/// Read the focused editor (fly) camera's world-space pose.
///
/// The `EditorCamera` marker rides whichever viewport is focused, and that
/// camera is never parented, so its `GlobalTransform` is its world pose. Returns
/// `None` if no editor camera currently carries the marker (e.g. mid-relocate or
/// during play mode).
pub fn editor_camera_world_pose(world: &mut World) -> Option<GlobalTransform> {
    let mut q =
        world.query_filtered::<&GlobalTransform, With<renzora::core::EditorCamera>>();
    q.iter(world).next().copied()
}

/// The focused editor camera's pose expressed in `entity`'s **local** space —
/// i.e. the `Transform` you'd assign to `entity` to make it sit exactly where
/// the editor fly-camera is.
///
/// Parent-aware: if the target is parented, the editor camera's world pose is
/// converted into the target's local space (a naive world→local copy would land
/// the camera at the wrong place under any non-identity parent — the original
/// cause of "snap doesn't move the camera" for imported/rigged cameras). Scale
/// is left at one; cameras don't use it.
///
/// Returns `None` if no editor camera currently carries the marker.
pub fn editor_camera_local_pose_for(world: &mut World, entity: Entity) -> Option<Transform> {
    let editor_world = editor_camera_world_pose(world)?;
    // Resolve the parent's world transform (identity if unparented).
    let parent_world = world
        .get::<ChildOf>(entity)
        .map(|c| c.parent())
        .and_then(|p| world.get::<GlobalTransform>(p).copied())
        .unwrap_or(GlobalTransform::IDENTITY);
    let local = parent_world.affine().inverse() * editor_world.affine();
    let m = Transform::from_matrix(local.into());
    Some(Transform {
        translation: m.translation,
        rotation: m.rotation,
        scale: Vec3::ONE,
    })
}

/// Move `entity` so its world-space pose matches the focused editor camera —
/// the "Snap to Viewport" action shared by the hierarchy context menu and the
/// Camera component button. Emits a toast either way so the action is never a
/// silent no-op. Returns `true` if the target moved.
pub fn snap_entity_to_editor_camera(world: &mut World, entity: Entity) -> bool {
    let Some(pose) = editor_camera_local_pose_for(world, entity) else {
        if let Some(mut toasts) = world.get_resource_mut::<renzora_ui::Toasts>() {
            toasts.warning("No active editor camera to snap to");
        }
        return false;
    };

    let Some(mut transform) = world.get_mut::<Transform>(entity) else {
        if let Some(mut toasts) = world.get_resource_mut::<renzora_ui::Toasts>() {
            toasts.warning("Selected entity has no transform to snap");
        }
        return false;
    };
    transform.translation = pose.translation;
    transform.rotation = pose.rotation;

    if let Some(mut toasts) = world.get_resource_mut::<renzora_ui::Toasts>() {
        toasts.info("Snapped camera to viewport");
    }
    true
}

/// Spawns the 2D camera the bevy_ui editor shell renders onto.
pub fn spawn_ui_camera(mut commands: Commands) {
    bevy::log::info!("[editor] Spawning UI camera");
    commands.spawn((
        Camera2d,
        Camera {
            order: 100,
            ..default()
        },
        EditorUiCamera,
        // Make this the default target for bevy_ui roots that don't set their
        // own `UiTargetCamera`. The bevy_ui editor shell (`renzora_shell`)
        // renders onto this existing camera so we don't add a second active
        // window camera (which trips bevy_pbr's atmosphere-probe extraction).
        bevy::ui::IsDefaultUiCamera,
    ));
}
