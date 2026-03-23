//! VR head → Kira spatial audio listener bridge
//!
//! Syncs the VR head pose to the entity with `AudioListenerData`,
//! ensuring spatial audio correctly tracks the user's head position
//! and rotation in VR.

use bevy::prelude::*;

use crate::camera::VrHead;

/// System: copy VR head global transform to the audio listener entity.
///
/// Runs before the Kira spatial audio sync system so that the listener
/// position is up-to-date when Kira processes spatial audio sources.
///
/// If no entity in the scene has `AudioListenerData`, this system does nothing.
/// The audio system's own listener sync will handle the non-VR case.
pub fn sync_vr_head_to_audio_listener(
    head_query: Query<&GlobalTransform, With<VrHead>>,
    // We match the audio listener by checking for the AudioListenerData component.
    // Since AudioListenerData lives in the main crate, we use a marker approach:
    // the VR play mode system attaches VrAudioListenerBridge to the listener entity.
    mut bridge_query: Query<&mut Transform, With<VrAudioListenerBridge>>,
) {
    let Ok(head_gt) = head_query.single() else {
        return;
    };

    for mut listener_tf in bridge_query.iter_mut() {
        // Copy the head's global transform to the listener's local transform
        // This works because the listener entity is typically at root level
        let (_, rotation, translation) = head_gt.to_scale_rotation_translation();
        listener_tf.translation = translation;
        listener_tf.rotation = rotation;
    }
}

/// Marker component: bridges VR head tracking to the audio listener entity.
/// Attached during VR play mode setup to the entity with `AudioListenerData`.
#[derive(Component, Default)]
pub struct VrAudioListenerBridge;
