//! Publish the wands' pointing rays into the world-UI pointer contract.
//!
//! Each frame both tracked wands contribute a [`renzora::WorldUiPointerRay`]
//! (grip pose, -Z forward, trigger value) to [`renzora::WorldUiPointers`].
//! The world-UI panel system (renzora_ember game_ui) intersects them with
//! `WorldUiPanel` quads and drives bevy_picking pointers, making in-world
//! `.html` UI clickable from the controllers — the same bridge the upcoming
//! in-headset editor panels ride.

use bevy::prelude::*;
use bevy_xr_utils::tracking_utils::{XrTrackedLeftGrip, XrTrackedRightGrip};

use crate::VrInput;

pub(crate) fn register(app: &mut App) {
    app.init_resource::<renzora::WorldUiPointers>();
    app.add_systems(Update, publish_wand_rays);
}

fn publish_wand_rays(
    input: Res<VrInput>,
    play: Option<Res<renzora::VrPlayState>>,
    left: Query<&GlobalTransform, With<XrTrackedLeftGrip>>,
    right: Query<&GlobalTransform, With<XrTrackedRightGrip>>,
    mut pointers: ResMut<renzora::WorldUiPointers>,
) {
    // Own only the wand ids — the mouse publisher owns id 2 (multiple
    // producers share the resource by replacing their own entries).
    pointers.0.retain(|r| r.id != 0 && r.id != 1);
    if !play.is_some_and(|p| p.active) {
        return;
    }
    let mut push = |id: u8, transform: Option<&GlobalTransform>, trigger: f32| {
        if let Some(tf) = transform {
            pointers.0.push(renzora::WorldUiPointerRay {
                id,
                ray: bevy::math::Ray3d::new(tf.translation(), tf.forward()),
                trigger,
            });
        }
    };
    push(0, left.iter().next(), input.left_trigger);
    push(1, right.iter().next(), input.right_trigger);
}
