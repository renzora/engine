//! VR controller and hand tracking input
//!
//! Reads OpenXR action sets each frame and populates `VrControllerState`
//! and `VrHandTrackingState` resources that the rest of the engine can query.

use bevy::prelude::*;
use bevy_xr_utils::actions::XRUtilsActionState;
use bevy_mod_xr::hands::{LeftHand, RightHand, XrHandBoneEntities, XrHandBoneRadius, HAND_JOINT_COUNT};
use bevy_mod_xr::spaces::XrSpaceLocationFlags;

use crate::VrHand;
use crate::actions::*;

/// Per-hand controller data
#[derive(Clone, Debug, Default)]
pub struct ControllerHandState {
    /// Grip pose (position + rotation of the controller body)
    pub grip_position: Vec3,
    pub grip_rotation: Quat,
    /// Aim pose (where the controller is pointing — ray origin)
    pub aim_position: Vec3,
    pub aim_rotation: Quat,
    /// Trigger value (0.0 = released, 1.0 = fully pressed)
    pub trigger: f32,
    /// Trigger pressed (past threshold)
    pub trigger_pressed: bool,
    /// Grip/squeeze value
    pub grip: f32,
    /// Grip pressed
    pub grip_pressed: bool,
    /// Thumbstick X (-1.0 left to 1.0 right)
    pub thumbstick_x: f32,
    /// Thumbstick Y (-1.0 down to 1.0 up)
    pub thumbstick_y: f32,
    /// Thumbstick clicked
    pub thumbstick_clicked: bool,
    /// Primary button (A on right, X on left for Oculus)
    pub button_a: bool,
    /// Secondary button (B on right, Y on left for Oculus)
    pub button_b: bool,
    /// Menu button
    pub menu: bool,
    /// Whether this controller is currently tracked
    pub tracked: bool,
}

/// Current state of both VR controllers, updated each frame
#[derive(Resource, Clone, Debug, Default)]
pub struct VrControllerState {
    pub left: ControllerHandState,
    pub right: ControllerHandState,
}

impl VrControllerState {
    /// Get controller state for a hand
    pub fn hand(&self, hand: VrHand) -> &ControllerHandState {
        match hand {
            VrHand::Left => &self.left,
            VrHand::Right => &self.right,
        }
    }

    /// Get controller state for a hand by string name
    pub fn hand_by_name(&self, name: &str) -> &ControllerHandState {
        match name.to_lowercase().as_str() {
            "right" => &self.right,
            _ => &self.left,
        }
    }
}

/// Per-hand joint data for hand tracking
#[derive(Clone, Debug, Default)]
pub struct HandJointState {
    /// 26 joint transforms (XR_HAND_JOINT_COUNT)
    pub joints: Vec<Transform>,
    /// Pinch strength (thumb + index tip proximity, 0.0 - 1.0)
    pub pinch_strength: f32,
    /// Grab strength (all fingers curl, 0.0 - 1.0)
    pub grab_strength: f32,
    /// Whether hand tracking is active for this hand
    pub tracked: bool,
}

/// Hand tracking state for both hands
#[derive(Resource, Clone, Debug, Default)]
pub struct VrHandTrackingState {
    pub left: HandJointState,
    pub right: HandJointState,
}

impl VrHandTrackingState {
    pub fn hand(&self, hand: VrHand) -> &HandJointState {
        match hand {
            VrHand::Left => &self.left,
            VrHand::Right => &self.right,
        }
    }
}

/// System: sync controller state from OpenXR action sets.
///
/// Reads marker-tagged action entities for `XRUtilsActionState` and
/// pose marker entities for `Transform` to populate `VrControllerState`.
pub fn sync_controller_state(
    mut controller_state: ResMut<VrControllerState>,
    // Float actions (trigger, grip)
    trigger_query: Query<(&TriggerAction, &XRUtilsActionState)>,
    grip_query: Query<(&GripAction, &XRUtilsActionState)>,
    // Vector actions (thumbstick)
    thumbstick_query: Query<(&ThumbstickAction, &XRUtilsActionState)>,
    // Bool actions
    button_a_query: Query<(&ButtonAAction, &XRUtilsActionState)>,
    button_b_query: Query<(&ButtonBAction, &XRUtilsActionState)>,
    menu_query: Query<(&MenuAction, &XRUtilsActionState)>,
    thumbstick_click_query: Query<(&ThumbstickClickAction, &XRUtilsActionState)>,
    // Pose entities
    left_grip_pose: Query<&Transform, With<LeftGripPose>>,
    right_grip_pose: Query<&Transform, With<RightGripPose>>,
    left_aim_pose: Query<&Transform, With<LeftAimPose>>,
    right_aim_pose: Query<&Transform, With<RightAimPose>>,
) {
    let state = &mut *controller_state;
    let mut any_left_active = false;
    let mut any_right_active = false;

    // ── Trigger (float) ──
    for (action, action_state) in trigger_query.iter() {
        if let XRUtilsActionState::Float(ref f) = *action_state {
            let hand_state = match action.0 {
                VrHand::Left => { any_left_active |= f.is_active; &mut state.left }
                VrHand::Right => { any_right_active |= f.is_active; &mut state.right }
            };
            hand_state.trigger = f.current_state;
            hand_state.trigger_pressed = f.current_state > 0.5;
        }
    }

    // ── Grip (float) ──
    for (action, action_state) in grip_query.iter() {
        if let XRUtilsActionState::Float(ref f) = *action_state {
            let hand_state = match action.0 {
                VrHand::Left => { any_left_active |= f.is_active; &mut state.left }
                VrHand::Right => { any_right_active |= f.is_active; &mut state.right }
            };
            hand_state.grip = f.current_state;
            hand_state.grip_pressed = f.current_state > 0.5;
        }
    }

    // ── Thumbstick (vector) ──
    for (action, action_state) in thumbstick_query.iter() {
        if let XRUtilsActionState::Vector(ref v) = *action_state {
            let hand_state = match action.0 {
                VrHand::Left => { any_left_active |= v.is_active; &mut state.left }
                VrHand::Right => { any_right_active |= v.is_active; &mut state.right }
            };
            hand_state.thumbstick_x = v.current_state[0];
            hand_state.thumbstick_y = v.current_state[1];
        }
    }

    // ── Button A (bool) ──
    for (action, action_state) in button_a_query.iter() {
        if let XRUtilsActionState::Bool(ref b) = *action_state {
            let hand_state = match action.0 {
                VrHand::Left => { any_left_active |= b.is_active; &mut state.left }
                VrHand::Right => { any_right_active |= b.is_active; &mut state.right }
            };
            hand_state.button_a = b.current_state;
        }
    }

    // ── Button B (bool) ──
    for (action, action_state) in button_b_query.iter() {
        if let XRUtilsActionState::Bool(ref b) = *action_state {
            let hand_state = match action.0 {
                VrHand::Left => { any_left_active |= b.is_active; &mut state.left }
                VrHand::Right => { any_right_active |= b.is_active; &mut state.right }
            };
            hand_state.button_b = b.current_state;
        }
    }

    // ── Menu (bool) ──
    for (action, action_state) in menu_query.iter() {
        if let XRUtilsActionState::Bool(ref b) = *action_state {
            let hand_state = match action.0 {
                VrHand::Left => { any_left_active |= b.is_active; &mut state.left }
                VrHand::Right => { any_right_active |= b.is_active; &mut state.right }
            };
            hand_state.menu = b.current_state;
        }
    }

    // ── Thumbstick click (bool) ──
    for (action, action_state) in thumbstick_click_query.iter() {
        if let XRUtilsActionState::Bool(ref b) = *action_state {
            let hand_state = match action.0 {
                VrHand::Left => { any_left_active |= b.is_active; &mut state.left }
                VrHand::Right => { any_right_active |= b.is_active; &mut state.right }
            };
            hand_state.thumbstick_clicked = b.current_state;
        }
    }

    // ── Pose data ──
    if let Ok(transform) = left_grip_pose.single() {
        state.left.grip_position = transform.translation;
        state.left.grip_rotation = transform.rotation;
        any_left_active = true;
    }
    if let Ok(transform) = right_grip_pose.single() {
        state.right.grip_position = transform.translation;
        state.right.grip_rotation = transform.rotation;
        any_right_active = true;
    }
    if let Ok(transform) = left_aim_pose.single() {
        state.left.aim_position = transform.translation;
        state.left.aim_rotation = transform.rotation;
    }
    if let Ok(transform) = right_aim_pose.single() {
        state.right.aim_position = transform.translation;
        state.right.aim_rotation = transform.rotation;
    }

    // Set tracked based on whether any action was active
    state.left.tracked = any_left_active;
    state.right.tracked = any_right_active;
}

/// System: sync hand tracking state from OpenXR hand tracking extension.
///
/// Queries `HandTrackingPlugin`'s spawned bone entities and populates
/// `VrHandTrackingState` with joint transforms, pinch/grab strength.
pub fn sync_hand_tracking(
    mut hand_state: ResMut<VrHandTrackingState>,
    left_hand_query: Query<&XrHandBoneEntities, With<LeftHand>>,
    right_hand_query: Query<&XrHandBoneEntities, With<RightHand>>,
    bone_query: Query<(&Transform, Option<&XrHandBoneRadius>, Option<&XrSpaceLocationFlags>)>,
) {
    sync_hand_side(&mut hand_state.left, &left_hand_query, &bone_query);
    sync_hand_side(&mut hand_state.right, &right_hand_query, &bone_query);
}

fn sync_hand_side(
    state: &mut HandJointState,
    hand_query: &Query<&XrHandBoneEntities, impl bevy::ecs::query::QueryFilter>,
    bone_query: &Query<(&Transform, Option<&XrHandBoneRadius>, Option<&XrSpaceLocationFlags>)>,
) {
    let Ok(bone_entities) = hand_query.single() else {
        state.tracked = false;
        return;
    };

    // XrHandBoneEntities derefs to [Entity; HAND_JOINT_COUNT]
    let entities: &[Entity; HAND_JOINT_COUNT] = &*bone_entities;

    // Resize joints vec if needed
    state.joints.resize(HAND_JOINT_COUNT, Transform::default());

    let mut any_tracked = false;
    for (i, &entity) in entities.iter().enumerate().take(HAND_JOINT_COUNT) {
        if let Ok((transform, _radius, flags)) = bone_query.get(entity) {
            state.joints[i] = *transform;
            if let Some(flags) = flags {
                if flags.position_tracked {
                    any_tracked = true;
                }
            }
        }
    }

    state.tracked = any_tracked;

    if !any_tracked {
        return;
    }

    // Calculate pinch strength: thumb tip (joint 5) to index tip (joint 10) distance
    // Map 0-6cm → 1.0-0.0
    let thumb_tip = state.joints.get(5).map(|t| t.translation).unwrap_or_default();
    let index_tip = state.joints.get(10).map(|t| t.translation).unwrap_or_default();
    let pinch_dist = thumb_tip.distance(index_tip);
    state.pinch_strength = (1.0 - (pinch_dist / 0.06).min(1.0)).max(0.0);

    // Calculate grab strength: average fingertip-to-palm distances
    // Palm is joint 0, fingertips are 10 (index), 15 (middle), 20 (ring), 25 (little)
    // Map 0-12cm → 1.0-0.0
    let palm = state.joints.get(0).map(|t| t.translation).unwrap_or_default();
    let fingertip_indices = [10, 15, 20, 25];
    let mut total_dist = 0.0f32;
    let mut count = 0;
    for &idx in &fingertip_indices {
        if let Some(joint) = state.joints.get(idx) {
            total_dist += palm.distance(joint.translation);
            count += 1;
        }
    }
    if count > 0 {
        let avg_dist = total_dist / count as f32;
        state.grab_strength = (1.0 - (avg_dist / 0.12).min(1.0)).max(0.0);
    }
}
