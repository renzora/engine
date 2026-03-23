//! Haptic feedback for VR controllers
//!
//! Provides `HapticPulseEvent` and a system that processes them into
//! OpenXR haptic output actions.

use bevy::prelude::*;
use bevy_mod_openxr::session::OxrSession;

use crate::VrHand;
use crate::resources::{vr_info, vr_warn};

/// Message to trigger a haptic vibration on a VR controller.
///
/// Send this message from any system to pulse a controller's vibration motor.
#[derive(Message, Debug, Clone)]
pub struct HapticPulseEvent {
    /// Which hand to vibrate
    pub hand: VrHand,
    /// Vibration amplitude (0.0 - 1.0)
    pub amplitude: f32,
    /// Duration in seconds
    pub duration_secs: f32,
    /// Frequency in Hz (0 = runtime default)
    pub frequency: f32,
}

/// Resource storing raw OpenXR haptic action handles.
/// Created during session setup by `create_haptic_actions`.
#[derive(Resource)]
pub struct HapticActions {
    pub left: openxr::Action<openxr::Haptic>,
    pub right: openxr::Action<openxr::Haptic>,
    pub action_set: openxr::ActionSet,
}

/// Create haptic output actions via raw OpenXR API.
///
/// Runs once when the XR session is created.
pub fn create_haptic_actions(
    mut commands: Commands,
    instance: Option<Res<bevy_mod_openxr::resources::OxrInstance>>,
    session: Option<Res<OxrSession>>,
) {
    let Some(instance) = instance else { return };
    let Some(session) = session else { return };

    let oxr_instance = instance.as_ref();

    let haptic_set = match oxr_instance.create_action_set("renzora_haptics", "Renzora Haptics", 0) {
        Ok(set) => set,
        Err(e) => {
            vr_warn(format!("Failed to create haptic action set: {e}"));
            return;
        }
    };

    let left = haptic_set
        .create_action::<openxr::Haptic>("haptic_left", "Left Haptic", &[])
        .expect("Failed to create left haptic action");
    let right = haptic_set
        .create_action::<openxr::Haptic>("haptic_right", "Right Haptic", &[])
        .expect("Failed to create right haptic action");

    // Suggest bindings
    let profiles = [
        "/interaction_profiles/oculus/touch_controller",
        "/interaction_profiles/valve/index_controller",
        "/interaction_profiles/htc/vive_controller",
        "/interaction_profiles/khr/simple_controller",
    ];

    for profile in &profiles {
        if let Ok(profile_path) = oxr_instance.string_to_path(profile) {
            let bindings = vec![
                openxr::Binding::new(
                    &left,
                    oxr_instance.string_to_path("/user/hand/left/output/haptic").unwrap(),
                ),
                openxr::Binding::new(
                    &right,
                    oxr_instance.string_to_path("/user/hand/right/output/haptic").unwrap(),
                ),
            ];
            let _ = oxr_instance.suggest_interaction_profile_bindings(profile_path, &bindings);
        }
    }

    // Attach
    if let Err(e) = session.attach_action_sets(&[&haptic_set]) {
        vr_warn(format!("Failed to attach haptic action sets: {e}"));
    }

    commands.insert_resource(HapticActions {
        left,
        right,
        action_set: haptic_set,
    });

    vr_info("OpenXR haptic actions created for both hands");
}

/// Process haptic pulse events and fire them via OpenXR.
pub fn process_haptic_events(
    mut events: MessageReader<HapticPulseEvent>,
    haptic_actions: Option<Res<HapticActions>>,
    session: Option<Res<OxrSession>>,
    instance: Option<Res<bevy_mod_openxr::resources::OxrInstance>>,
) {
    let Some(haptics) = haptic_actions else { return };
    let Some(session) = session else { return };
    let Some(instance) = instance else { return };

    let oxr_instance = instance.as_ref();

    for event in events.read() {
        let action = match event.hand {
            VrHand::Left => &haptics.left,
            VrHand::Right => &haptics.right,
        };

        let subaction_path = match event.hand {
            VrHand::Left => "/user/hand/left",
            VrHand::Right => "/user/hand/right",
        };

        let path = match oxr_instance.string_to_path(subaction_path) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let duration_nanos = (event.duration_secs.max(0.0) * 1_000_000_000.0) as i64;
        let frequency = if event.frequency > 0.0 { event.frequency } else { 0.0 };

        // Use builder pattern for HapticVibration
        let vibration = openxr::HapticVibration::new()
            .duration(openxr::Duration::from_nanos(duration_nanos))
            .amplitude(event.amplitude.clamp(0.0, 1.0))
            .frequency(frequency);

        // apply_feedback is on Action<Haptic>, not on Session
        if let Err(e) = action.apply_feedback(&*session, path, &vibration) {
            vr_warn(format!("Haptic pulse failed for {:?}: {e}", event.hand));
        }
    }
}
