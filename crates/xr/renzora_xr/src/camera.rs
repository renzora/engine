//! VR camera rig — root entity + head tracking
//!
//! `bevy_mod_openxr` handles the actual stereo eye cameras and swapchain.
//! We provide a `VrCameraRig` root entity that can be teleported/moved,
//! and a `VrHead` child whose transform is synced from OpenXR head tracking.

use bevy::prelude::*;
use bevy_mod_openxr::resources::OxrViews;
use crate::resources::vr_info;

/// Marker component for the VR camera rig root entity.
/// This is the entity that gets moved by locomotion (teleport/smooth).
#[derive(Component, Default, Debug)]
pub struct VrCameraRig;

/// Marker component for the VR head entity (child of rig).
/// Transform is synced from OpenXR head tracking each frame.
#[derive(Component, Default, Debug)]
pub struct VrHead;

/// Message: request to spawn the VR camera rig at a specific position
#[derive(Message)]
pub struct SpawnVrCameraRigEvent {
    pub position: Vec3,
    pub rotation: Quat,
}

/// Message: request to despawn the VR camera rig
#[derive(Message)]
pub struct DespawnVrCameraRigEvent;

/// System: spawn VR camera rig when event is received.
///
/// Creates a root entity (VrCameraRig) at the scene's default camera position,
/// with a child entity (VrHead) that tracks the headset pose.
pub fn spawn_vr_camera_rig(
    mut commands: Commands,
    mut events: MessageReader<SpawnVrCameraRigEvent>,
    existing_rigs: Query<Entity, With<VrCameraRig>>,
) {
    for event in events.read() {
        // Don't spawn duplicates
        if !existing_rigs.is_empty() {
            continue;
        }

        vr_info(format!("Spawning VR camera rig at {:?}", event.position));

        commands
            .spawn((
                VrCameraRig,
                Transform::from_translation(event.position)
                    .with_rotation(event.rotation),
                Visibility::default(),
                Name::new("VR Camera Rig"),
            ))
            .with_children(|parent| {
                parent.spawn((
                    VrHead,
                    Transform::default(),
                    Visibility::default(),
                    Name::new("VR Head"),
                ));
            });
    }
}

/// System: despawn VR camera rig when event is received.
pub fn despawn_vr_camera_rig(
    mut commands: Commands,
    mut events: MessageReader<DespawnVrCameraRigEvent>,
    rig_query: Query<Entity, With<VrCameraRig>>,
) {
    for _event in events.read() {
        for entity in rig_query.iter() {
            vr_info("Despawning VR camera rig");
            commands.entity(entity).despawn();
        }
    }
}

/// System: sync VrHead transform from OpenXR head/view tracking.
///
/// Reads `OxrViews` to get the head pose and applies it to the `VrHead` entity
/// relative to the `VrCameraRig` parent.
pub fn sync_vr_head(
    views: Option<Res<OxrViews>>,
    mut head_query: Query<&mut Transform, With<VrHead>>,
) {
    let Some(views) = views else { return };

    // Average the two eye views to get the head center pose
    if views.is_empty() {
        return;
    }

    // Use the first view's pose as the head pose (center-eye approximation)
    // For more accuracy, average left and right eye positions
    let head_transform = if views.len() >= 2 {
        let lp = &views[0].pose.position;
        let rp = &views[1].pose.position;
        let pos = Vec3::new(
            (lp.x + rp.x) * 0.5,
            (lp.y + rp.y) * 0.5,
            (lp.z + rp.z) * 0.5,
        );
        let lo = &views[0].pose.orientation;
        let ro = &views[1].pose.orientation;
        let left_quat = Quat::from_xyzw(lo.x, lo.y, lo.z, lo.w);
        let right_quat = Quat::from_xyzw(ro.x, ro.y, ro.z, ro.w);
        let rot = left_quat.slerp(right_quat, 0.5);
        Transform::from_translation(pos).with_rotation(rot)
    } else {
        let p = &views[0].pose.position;
        let o = &views[0].pose.orientation;
        Transform::from_translation(Vec3::new(p.x, p.y, p.z))
            .with_rotation(Quat::from_xyzw(o.x, o.y, o.z, o.w))
    };

    for mut transform in head_query.iter_mut() {
        *transform = head_transform;
    }
}
