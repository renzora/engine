//! OpenXR action sets and bindings for VR controller input
//!
//! Uses bevy_xr_utils entity-based approach for button/axis actions,
//! and raw openxr API for pose actions (not supported by bevy_xr_utils).

use bevy::prelude::*;
use bevy_xr_utils::actions::{
    ActiveSet, ActionType, XRUtilsAction, XRUtilsActionSet, XRUtilsActionSetReference, XRUtilsBinding,
    Actionf32Reference, ActionBooleference, ActionVector2fReference,
};
use bevy_mod_openxr::action_binding::OxrSuggestActionBinding;
use bevy_mod_openxr::action_set_attaching::OxrAttachActionSet;
use bevy_mod_openxr::resources::OxrInstance;
use bevy_mod_openxr::session::OxrSession;
use bevy_mod_xr::spaces::XrSpaceLocationFlags;

use crate::VrHand;
use crate::resources::{vr_info, vr_warn};

// ── Marker components for action entity queries ──

#[derive(Component)]
pub struct TriggerAction(pub VrHand);

#[derive(Component)]
pub struct GripAction(pub VrHand);

#[derive(Component)]
pub struct ThumbstickAction(pub VrHand);

#[derive(Component)]
pub struct ButtonAAction(pub VrHand);

#[derive(Component)]
pub struct ButtonBAction(pub VrHand);

#[derive(Component)]
pub struct MenuAction(pub VrHand);

#[derive(Component)]
pub struct ThumbstickClickAction(pub VrHand);

// ── Pose marker components ──

#[derive(Component)]
pub struct LeftAimPose;

#[derive(Component)]
pub struct RightAimPose;

#[derive(Component)]
pub struct LeftGripPose;

#[derive(Component)]
pub struct RightGripPose;

/// Resource storing the action set entity for later reference
#[derive(Resource)]
pub struct RenzoraActionSetEntity(pub Entity);

/// Interaction profile paths for controller binding
const OCULUS_TOUCH: &str = "/interaction_profiles/oculus/touch_controller";
const VALVE_INDEX: &str = "/interaction_profiles/valve/index_controller";
const HTC_VIVE: &str = "/interaction_profiles/htc/vive_controller";
const KHR_SIMPLE: &str = "/interaction_profiles/khr/simple_controller";

/// Spawn the action set entity hierarchy during Startup.
///
/// Creates `XRUtilsActionSet` "renzora_input" with child action entities
/// for triggers, grips, thumbsticks, buttons, and menu. Each action entity
/// gets grandchild `XRUtilsBinding` entities for supported controller profiles.
pub fn setup_action_sets(mut commands: Commands) {
    let action_set = commands
        .spawn((
            XRUtilsActionSet {
                name: "renzora_input".into(),
                pretty_name: "Renzora Input".into(),
                priority: 0,
            },
            ActiveSet,
            Name::new("Renzora Input Action Set"),
        ))
        .id();

    commands.insert_resource(RenzoraActionSetEntity(action_set));

    // Helper: spawn an action entity as child of action_set with bindings
    macro_rules! spawn_action {
        ($commands:expr, $parent:expr, $name:expr, $pretty:expr, $action_type:expr,
         $marker:expr, $( $profile:expr => $path:expr ),+ $(,)?) => {{
            let action = $commands
                .spawn((
                    XRUtilsAction {
                        action_name: $name.into(),
                        localized_name: $pretty.into(),
                        action_type: $action_type,
                    },
                    $marker,
                    Name::new($pretty),
                ))
                .id();
            $commands.entity($parent).add_child(action);

            $(
                let binding = $commands
                    .spawn(XRUtilsBinding {
                        profile: $profile.into(),
                        binding: $path.into(),
                    })
                    .id();
                $commands.entity(action).add_child(binding);
            )+

            action
        }};
    }

    // ── Float actions ──

    // Trigger left
    spawn_action!(commands, action_set, "trigger_left", "Left Trigger",
        ActionType::Float,
        TriggerAction(VrHand::Left),
        OCULUS_TOUCH => "/user/hand/left/input/trigger/value",
        VALVE_INDEX  => "/user/hand/left/input/trigger/value",
        HTC_VIVE     => "/user/hand/left/input/trigger/value",
        KHR_SIMPLE   => "/user/hand/left/input/select/click",
    );

    // Trigger right
    spawn_action!(commands, action_set, "trigger_right", "Right Trigger",
        ActionType::Float,
        TriggerAction(VrHand::Right),
        OCULUS_TOUCH => "/user/hand/right/input/trigger/value",
        VALVE_INDEX  => "/user/hand/right/input/trigger/value",
        HTC_VIVE     => "/user/hand/right/input/trigger/value",
        KHR_SIMPLE   => "/user/hand/right/input/select/click",
    );

    // Grip left
    spawn_action!(commands, action_set, "grip_left", "Left Grip",
        ActionType::Float,
        GripAction(VrHand::Left),
        OCULUS_TOUCH => "/user/hand/left/input/squeeze/value",
        VALVE_INDEX  => "/user/hand/left/input/squeeze/value",
        HTC_VIVE     => "/user/hand/left/input/squeeze/click",
    );

    // Grip right
    spawn_action!(commands, action_set, "grip_right", "Right Grip",
        ActionType::Float,
        GripAction(VrHand::Right),
        OCULUS_TOUCH => "/user/hand/right/input/squeeze/value",
        VALVE_INDEX  => "/user/hand/right/input/squeeze/value",
        HTC_VIVE     => "/user/hand/right/input/squeeze/click",
    );

    // ── Vector actions (thumbsticks) ──

    // Thumbstick left
    spawn_action!(commands, action_set, "thumbstick_left", "Left Thumbstick",
        ActionType::Vector,
        ThumbstickAction(VrHand::Left),
        OCULUS_TOUCH => "/user/hand/left/input/thumbstick",
        VALVE_INDEX  => "/user/hand/left/input/thumbstick",
        HTC_VIVE     => "/user/hand/left/input/trackpad",
    );

    // Thumbstick right
    spawn_action!(commands, action_set, "thumbstick_right", "Right Thumbstick",
        ActionType::Vector,
        ThumbstickAction(VrHand::Right),
        OCULUS_TOUCH => "/user/hand/right/input/thumbstick",
        VALVE_INDEX  => "/user/hand/right/input/thumbstick",
        HTC_VIVE     => "/user/hand/right/input/trackpad",
    );

    // ── Bool actions ──

    // Button A left (X on Oculus)
    spawn_action!(commands, action_set, "button_a_left", "Left Button A",
        ActionType::Bool,
        ButtonAAction(VrHand::Left),
        OCULUS_TOUCH => "/user/hand/left/input/x/click",
        VALVE_INDEX  => "/user/hand/left/input/a/click",
    );

    // Button A right
    spawn_action!(commands, action_set, "button_a_right", "Right Button A",
        ActionType::Bool,
        ButtonAAction(VrHand::Right),
        OCULUS_TOUCH => "/user/hand/right/input/a/click",
        VALVE_INDEX  => "/user/hand/right/input/a/click",
    );

    // Button B left (Y on Oculus)
    spawn_action!(commands, action_set, "button_b_left", "Left Button B",
        ActionType::Bool,
        ButtonBAction(VrHand::Left),
        OCULUS_TOUCH => "/user/hand/left/input/y/click",
        VALVE_INDEX  => "/user/hand/left/input/b/click",
    );

    // Button B right
    spawn_action!(commands, action_set, "button_b_right", "Right Button B",
        ActionType::Bool,
        ButtonBAction(VrHand::Right),
        OCULUS_TOUCH => "/user/hand/right/input/b/click",
        VALVE_INDEX  => "/user/hand/right/input/b/click",
    );

    // Menu left
    spawn_action!(commands, action_set, "menu_left", "Left Menu",
        ActionType::Bool,
        MenuAction(VrHand::Left),
        OCULUS_TOUCH => "/user/hand/left/input/menu/click",
        VALVE_INDEX  => "/user/hand/left/input/system/click",
        HTC_VIVE     => "/user/hand/left/input/menu/click",
        KHR_SIMPLE   => "/user/hand/left/input/menu/click",
    );

    // Thumbstick click left
    spawn_action!(commands, action_set, "thumbstick_click_left", "Left Thumbstick Click",
        ActionType::Bool,
        ThumbstickClickAction(VrHand::Left),
        OCULUS_TOUCH => "/user/hand/left/input/thumbstick/click",
        VALVE_INDEX  => "/user/hand/left/input/thumbstick/click",
        HTC_VIVE     => "/user/hand/left/input/trackpad/click",
    );

    // Thumbstick click right
    spawn_action!(commands, action_set, "thumbstick_click_right", "Right Thumbstick Click",
        ActionType::Bool,
        ThumbstickClickAction(VrHand::Right),
        OCULUS_TOUCH => "/user/hand/right/input/thumbstick/click",
        VALVE_INDEX  => "/user/hand/right/input/thumbstick/click",
        HTC_VIVE     => "/user/hand/right/input/trackpad/click",
    );

    vr_info("OpenXR action sets configured with Oculus/Index/Vive/Simple bindings");
}

/// Add pose actions to the existing action set created by XRUtilsActionsPlugin.
///
/// **Exclusive system** — takes `&mut World` so `PoseActionHandles` is inserted
/// immediately (not via deferred commands). This prevents the system from
/// re-running and hitting "session already has attached action sets".
///
/// Runs each frame until it succeeds (idempotent). `create_openxr_events`
/// inserts `XRUtilsActionSetReference` via deferred commands during Startup,
/// so it may not be visible until the next frame.
pub fn add_pose_actions_to_set(world: &mut World) {
    // Already done — don't re-create
    if world.get_resource::<PoseActionHandles>().is_some() { return; }

    let Some(instance) = world.get_resource::<OxrInstance>() else { return };
    let instance = instance.clone();

    // Find the action set reference — query manually from World
    let mut set_ref_clone: Option<openxr::ActionSet> = None;
    let mut query = world.query_filtered::<&XRUtilsActionSetReference, With<ActiveSet>>();
    for r in query.iter(world) {
        set_ref_clone = Some(r.0.clone());
        break;
    }
    let Some(action_set) = set_ref_clone else {
        // Not available yet — will retry next frame
        return;
    };

    // Create pose actions on the existing action set
    let aim_left = match action_set.create_action::<openxr::Posef>("aim_left", "Left Aim Pose", &[]) {
        Ok(a) => a,
        Err(e) => { vr_warn(format!("Failed to create aim_left action: {e}")); return; }
    };
    let aim_right = match action_set.create_action::<openxr::Posef>("aim_right", "Right Aim Pose", &[]) {
        Ok(a) => a,
        Err(e) => { vr_warn(format!("Failed to create aim_right action: {e}")); return; }
    };
    let grip_pose_left = match action_set.create_action::<openxr::Posef>("grip_pose_left", "Left Grip Pose", &[]) {
        Ok(a) => a,
        Err(e) => { vr_warn(format!("Failed to create grip_pose_left action: {e}")); return; }
    };
    let grip_pose_right = match action_set.create_action::<openxr::Posef>("grip_pose_right", "Right Grip Pose", &[]) {
        Ok(a) => a,
        Err(e) => { vr_warn(format!("Failed to create grip_pose_right action: {e}")); return; }
    };

    // Insert resource IMMEDIATELY (not deferred) — this is why we use an exclusive system.
    world.insert_resource(PoseActionHandles {
        aim_left,
        aim_right,
        grip_left: grip_pose_left,
        grip_right: grip_pose_right,
    });

    vr_info("Pose actions created on action set (exclusive system — immediate insert)");
}

/// Re-send pose binding suggestions for `OxrSendActionBindings` schedule.
///
/// Pose bindings need to be suggested right before action set attachment
/// during session creation. Uses `OxrSuggestActionBinding` messages to go
/// through the same pipeline as button/axis bindings.
pub fn resend_pose_bindings(
    handles: Option<Res<PoseActionHandles>>,
    mut binding_writer: MessageWriter<OxrSuggestActionBinding>,
) {
    let Some(handles) = handles else { return; };

    let profiles = [OCULUS_TOUCH, VALVE_INDEX, HTC_VIVE, KHR_SIMPLE];

    let bindings_per_action: &[(&openxr::Action<openxr::Posef>, &str)] = &[
        (&handles.aim_left,   "/user/hand/left/input/aim/pose"),
        (&handles.aim_right,  "/user/hand/right/input/aim/pose"),
        (&handles.grip_left,  "/user/hand/left/input/grip/pose"),
        (&handles.grip_right, "/user/hand/right/input/grip/pose"),
    ];

    for profile in &profiles {
        for (action, path) in bindings_per_action {
            binding_writer.write(OxrSuggestActionBinding {
                action: action.as_raw(),
                interaction_profile: (*profile).into(),
                bindings: vec![(*path).into()],
            });
        }
    }
}

/// Re-send button/axis binding suggestions for the `OxrSendActionBindings` schedule.
///
/// `create_openxr_events` writes binding suggestions as messages during Startup,
/// but messages expire after 1–2 frames. With manual session handling, the session
/// is created much later, so bindings are lost. This system runs inside the
/// `OxrSendActionBindings` schedule (triggered at session creation) to re-send
/// them fresh.
pub fn resend_button_bindings(
    action_f32_query: Query<(&Actionf32Reference, &Children)>,
    action_bool_query: Query<(&ActionBooleference, &Children)>,
    action_vec_query: Query<(&ActionVector2fReference, &Children)>,
    bindings_query: Query<&XRUtilsBinding>,
    mut binding_writer: MessageWriter<OxrSuggestActionBinding>,
) {
    // Re-send f32 action bindings (trigger, grip)
    for (action_ref, children) in action_f32_query.iter() {
        for child in children.iter() {
            if let Ok(binding) = bindings_query.get(child) {
                binding_writer.write(OxrSuggestActionBinding {
                    action: action_ref.action.as_raw(),
                    interaction_profile: binding.profile.clone(),
                    bindings: vec![binding.binding.clone()],
                });
            }
        }
    }

    // Re-send bool action bindings (buttons, menu)
    for (action_ref, children) in action_bool_query.iter() {
        for child in children.iter() {
            if let Ok(binding) = bindings_query.get(child) {
                binding_writer.write(OxrSuggestActionBinding {
                    action: action_ref.action.as_raw(),
                    interaction_profile: binding.profile.clone(),
                    bindings: vec![binding.binding.clone()],
                });
            }
        }
    }

    // Re-send vector action bindings (thumbsticks)
    for (action_ref, children) in action_vec_query.iter() {
        for child in children.iter() {
            if let Ok(binding) = bindings_query.get(child) {
                binding_writer.write(OxrSuggestActionBinding {
                    action: action_ref.action.as_raw(),
                    interaction_profile: binding.profile.clone(),
                    bindings: vec![binding.binding.clone()],
                });
            }
        }
    }
}

/// Re-send action set attachment for `OxrSendActionBindings` schedule.
///
/// Same issue as bindings: the `OxrAttachActionSet` message from Startup expires.
pub fn resend_action_set_attachment(
    action_set_query: Query<&XRUtilsActionSetReference, With<ActiveSet>>,
    mut attach_writer: MessageWriter<OxrAttachActionSet>,
) {
    for set_ref in action_set_query.iter() {
        attach_writer.write(OxrAttachActionSet(set_ref.0.clone()));
    }
}

/// Create action spaces for pose actions after the session is created.
///
/// Runs in `XrSessionCreated` schedule. Needs the session to exist so
/// we can call `create_action_space`.
pub fn create_pose_spaces(
    mut commands: Commands,
    session: Res<OxrSession>,
    instance: Res<OxrInstance>,
    handles: Option<Res<PoseActionHandles>>,
) {
    let Some(handles) = handles else {
        vr_warn("No pose action handles — cannot create pose spaces");
        return;
    };

    macro_rules! create_space_entity {
        ($action:expr, $subaction:expr, $marker:expr, $name:expr) => {
            match instance.string_to_path($subaction) {
                Ok(subaction_path) => {
                    match session.create_action_space(
                        &$action,
                        subaction_path,
                        bevy::math::Isometry3d::IDENTITY,
                    ) {
                        Ok(space) => {
                            // XrSpaceLocationFlags is required by update_space_transforms.
                            // OxrSpaceLocationFlags is auto-added as a required component.
                            commands.spawn((
                                space,
                                Transform::default(),
                                XrSpaceLocationFlags::default(),
                                $marker,
                                Name::new($name),
                            ));
                        }
                        Err(e) => vr_warn(format!("Failed to create {} space: {e}", $name)),
                    }
                }
                Err(e) => vr_warn(format!("Failed to convert path for {}: {e}", $name)),
            }
        };
    }

    create_space_entity!(handles.aim_left,   "/user/hand/left",  LeftAimPose,   "Left Aim Pose");
    create_space_entity!(handles.aim_right,  "/user/hand/right", RightAimPose,  "Right Aim Pose");
    create_space_entity!(handles.grip_left,  "/user/hand/left",  LeftGripPose,  "Left Grip Pose");
    create_space_entity!(handles.grip_right, "/user/hand/right", RightGripPose, "Right Grip Pose");

    vr_info("OpenXR pose spaces created (aim + grip for both hands)");
}

/// Stores raw OpenXR pose action handles between Startup and session creation.
#[derive(Resource)]
pub struct PoseActionHandles {
    pub aim_left: openxr::Action<openxr::Posef>,
    pub aim_right: openxr::Action<openxr::Posef>,
    pub grip_left: openxr::Action<openxr::Posef>,
    pub grip_right: openxr::Action<openxr::Posef>,
}
