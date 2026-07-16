//! OpenXR action-set setup + per-frame controller state.
//!
//! One Renzora action set with per-hand actions (bevy_xr_utils syncs actions
//! with `Path::NULL`, so left/right must be *separate actions* rather than one
//! action with two sub-action bindings). States are mirrored each frame into
//! the [`VrInput`] resource, which is the surface gameplay/locomotion code
//! reads — nothing outside this module touches `XRUtilsActionState` directly.

use std::borrow::Cow;

use bevy::prelude::*;
use bevy_xr_utils::actions::{
    ActionType, XRUtilsAction, XRUtilsActionSet, XRUtilsActionState, XRUtilsActionSystems,
    XRUtilsBinding,
};

/// Controller input state — updated every frame from OpenXR actions.
#[derive(Resource, Default, Debug)]
pub struct VrInput {
    pub left_thumbstick: Vec2,
    pub right_thumbstick: Vec2,
    pub left_trigger: f32,
    pub right_trigger: f32,
    pub left_grip: f32,
    pub right_grip: f32,
    pub button_a: bool,
    pub button_b: bool,
    pub button_x: bool,
    pub button_y: bool,
    pub menu: bool,
}

/// Which [`VrInput`] field an action entity feeds (avoids string matching in
/// the per-frame sync).
#[derive(Component, Clone, Copy)]
enum VrActionSlot {
    LeftThumbstick,
    RightThumbstick,
    LeftTrigger,
    RightTrigger,
    LeftGrip,
    RightGrip,
    ButtonA,
    ButtonB,
    ButtonX,
    ButtonY,
    Menu,
}

pub(crate) fn register(app: &mut App) {
    app.add_systems(
        Startup,
        setup_action_sets.before(XRUtilsActionSystems::CreateEvents),
    );
    app.add_systems(
        Update,
        sync_input.after(XRUtilsActionSystems::SyncActionStates),
    );
}

/// Declare the Renzora action set with Oculus Touch bindings. Runtimes for
/// other controllers (Index, WMR, Vive) rebind through the OpenXR runtime's
/// own remapping layer until explicit profiles are added here.
fn setup_action_sets(mut commands: Commands) {
    const OCULUS: &str = "/interaction_profiles/oculus/touch_controller";

    let set = commands
        .spawn(XRUtilsActionSet {
            name: "renzora_vr".into(),
            pretty_name: "Renzora VR Controls".into(),
            priority: 0,
        })
        .id();

    let actions: &[(&str, ActionType, VrActionSlot, &str)] = &[
        ("left_thumbstick", ActionType::Vector, VrActionSlot::LeftThumbstick, "/user/hand/left/input/thumbstick"),
        ("right_thumbstick", ActionType::Vector, VrActionSlot::RightThumbstick, "/user/hand/right/input/thumbstick"),
        ("left_trigger", ActionType::Float, VrActionSlot::LeftTrigger, "/user/hand/left/input/trigger/value"),
        ("right_trigger", ActionType::Float, VrActionSlot::RightTrigger, "/user/hand/right/input/trigger/value"),
        ("left_grip", ActionType::Float, VrActionSlot::LeftGrip, "/user/hand/left/input/squeeze/value"),
        ("right_grip", ActionType::Float, VrActionSlot::RightGrip, "/user/hand/right/input/squeeze/value"),
        ("button_a", ActionType::Bool, VrActionSlot::ButtonA, "/user/hand/right/input/a/click"),
        ("button_b", ActionType::Bool, VrActionSlot::ButtonB, "/user/hand/right/input/b/click"),
        ("button_x", ActionType::Bool, VrActionSlot::ButtonX, "/user/hand/left/input/x/click"),
        ("button_y", ActionType::Bool, VrActionSlot::ButtonY, "/user/hand/left/input/y/click"),
        ("menu", ActionType::Bool, VrActionSlot::Menu, "/user/hand/left/input/menu/click"),
    ];

    for (name, action_type, slot, binding) in actions {
        let action = commands
            .spawn((
                XRUtilsAction {
                    action_name: (*name).into(),
                    localized_name: (*name).into(),
                    action_type: *action_type,
                },
                *slot,
            ))
            .set_parent_in_place(set)
            .id();
        commands
            .spawn(XRUtilsBinding {
                profile: Cow::Borrowed(OCULUS),
                binding: Cow::Borrowed(*binding),
            })
            .set_parent_in_place(action);
    }
}

/// Mirror OpenXR action states into [`VrInput`].
fn sync_input(actions: Query<(&VrActionSlot, &XRUtilsActionState)>, mut input: ResMut<VrInput>) {
    for (slot, state) in actions.iter() {
        match (slot, state) {
            (VrActionSlot::LeftThumbstick, XRUtilsActionState::Vector(v)) => {
                input.left_thumbstick = Vec2::new(v.current_state[0], v.current_state[1]);
            }
            (VrActionSlot::RightThumbstick, XRUtilsActionState::Vector(v)) => {
                input.right_thumbstick = Vec2::new(v.current_state[0], v.current_state[1]);
            }
            (VrActionSlot::LeftTrigger, XRUtilsActionState::Float(f)) => {
                input.left_trigger = f.current_state;
            }
            (VrActionSlot::RightTrigger, XRUtilsActionState::Float(f)) => {
                input.right_trigger = f.current_state;
            }
            (VrActionSlot::LeftGrip, XRUtilsActionState::Float(f)) => {
                input.left_grip = f.current_state;
            }
            (VrActionSlot::RightGrip, XRUtilsActionState::Float(f)) => {
                input.right_grip = f.current_state;
            }
            (VrActionSlot::ButtonA, XRUtilsActionState::Bool(b)) => {
                input.button_a = b.current_state;
            }
            (VrActionSlot::ButtonB, XRUtilsActionState::Bool(b)) => {
                input.button_b = b.current_state;
            }
            (VrActionSlot::ButtonX, XRUtilsActionState::Bool(b)) => {
                input.button_x = b.current_state;
            }
            (VrActionSlot::ButtonY, XRUtilsActionState::Bool(b)) => {
                input.button_y = b.current_state;
            }
            (VrActionSlot::Menu, XRUtilsActionState::Bool(b)) => {
                input.menu = b.current_state;
            }
            _ => {}
        }
    }
}
