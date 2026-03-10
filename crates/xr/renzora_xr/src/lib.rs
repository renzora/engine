//! VR/XR support for Renzora Engine via OpenXR
//!
//! Provides stereo rendering, head tracking, controller input, hand tracking,
//! locomotion, grab interaction, haptic feedback, passthrough, overlay,
//! and spatial audio integration.
//!
//! Gated behind the `xr` feature flag in the root crate. Activated at runtime
//! with the `--vr` CLI flag.

pub mod session;
pub mod camera;
pub mod input;
pub mod interaction;
pub mod audio_bridge;
pub mod components;
pub mod resources;
pub mod actions;
pub mod haptics;
pub mod passthrough;
pub mod reference_space;
pub mod overlay;
pub mod extensions;

use bevy::prelude::*;
use bevy_xr_utils::actions::{XRUtilsActionsPlugin, XRUtilsActionSystems};
use serde::{Deserialize, Serialize};

use resources::VrSessionState;

// Re-export key types for external use
pub use camera::{VrCameraRig, VrHead};
pub use input::{VrControllerState, VrHandTrackingState};
pub use resources::VrModeActive;
pub use components::{VrControllerData, TeleportAreaData, VrGrabbableData};
pub use haptics::HapticPulseEvent;
pub use extensions::VrCapabilities;
pub use resources::{VrLogBuffer, VrLogLevel, VrLogEntry, init_vr_log_buffer, get_vr_log_buffer};

/// Master VR configuration resource.
/// Persisted per-project via the VR settings panel.
#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct VrConfig {
    /// Render resolution scale (0.5 = half res, 1.0 = native, 1.5 = supersampled)
    pub render_scale: f32,
    /// Comfort vignette intensity during locomotion (0.0 = off, 1.0 = full)
    pub comfort_vignette: f32,
    /// Snap turn angle in degrees (0 = smooth turning)
    pub snap_turn_angle: f32,
    /// Locomotion mode
    pub locomotion_mode: LocomotionMode,
    /// Smooth locomotion speed (m/s)
    pub move_speed: f32,
    /// Enable hand tracking (if headset supports it)
    pub hand_tracking_enabled: bool,
    /// Seated mode — adjusts reference space
    pub seated_mode: bool,
    /// Which hand controls locomotion
    pub locomotion_hand: VrHand,
    /// Deadzone for thumbstick inputs
    pub thumbstick_deadzone: f32,
    /// Snap turn cooldown in seconds
    pub snap_turn_cooldown: f32,
    /// Enable passthrough (mixed reality)
    pub passthrough_enabled: bool,
    /// Environment blend mode
    pub blend_mode: BlendMode,
    /// Enable foveated rendering (if supported)
    pub foveated_rendering: bool,
}

impl Default for VrConfig {
    fn default() -> Self {
        Self {
            render_scale: 1.0,
            comfort_vignette: 0.3,
            snap_turn_angle: 45.0,
            locomotion_mode: LocomotionMode::Teleport,
            move_speed: 4.0,
            hand_tracking_enabled: true,
            seated_mode: false,
            locomotion_hand: VrHand::Left,
            thumbstick_deadzone: 0.2,
            snap_turn_cooldown: 0.3,
            passthrough_enabled: false,
            blend_mode: BlendMode::Opaque,
            foveated_rendering: true,
        }
    }
}

/// Locomotion mode for VR movement
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocomotionMode {
    Teleport,
    Smooth,
    Both,
}

/// Environment blend mode for mixed reality
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendMode {
    /// Fully opaque VR (no passthrough)
    Opaque,
    /// Additive blending (holographic overlay)
    Additive,
    /// Alpha blending (mixed reality passthrough)
    AlphaBlend,
}

/// Which hand (left or right)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VrHand {
    #[default]
    Left,
    Right,
}

impl VrHand {
    pub fn as_str(&self) -> &'static str {
        match self {
            VrHand::Left => "left",
            VrHand::Right => "right",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "right" => VrHand::Right,
            _ => VrHand::Left,
        }
    }
}

/// Main XR plugin — registers all VR systems and resources.
/// Only does work when `VrModeActive` resource is present.
pub struct XrPlugin;

impl Plugin for XrPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] XrPlugin");
        app.init_resource::<VrConfig>()
            .init_resource::<VrControllerState>()
            .init_resource::<VrHandTrackingState>()
            .init_resource::<VrSessionState>();

        // Messages (Bevy 0.18 event system)
        app.add_message::<HapticPulseEvent>()
            .add_message::<resources::VrSessionStatusChanged>();

        // Session lifecycle
        app.add_systems(
            Update,
            (
                session::update_session_state,
                session::enumerate_refresh_rates,
                session::apply_requested_refresh_rate,
            )
                .chain()
                .run_if(resource_exists::<VrModeActive>),
        );

        // Camera rig management
        app.add_systems(
            Update,
            (
                camera::spawn_vr_camera_rig,
                camera::despawn_vr_camera_rig,
                camera::sync_vr_head,
            )
                .chain()
                .run_if(resource_exists::<VrModeActive>),
        );

        // Action sets (startup) — must run BEFORE XRUtilsActionsPlugin::CreateEvents
        // so the entity hierarchy exists when create_openxr_events reads it.
        app.add_systems(
            Startup,
            actions::setup_action_sets
                .before(XRUtilsActionSystems::CreateEvents),
        );
        // Pose actions need the raw ActionSet created by CreateEvents.
        // CreateEvents inserts XRUtilsActionSetReference via deferred commands,
        // so it may not be visible in the same Startup frame. Run in Update
        // (idempotent — stops after first success) to ensure commands are flushed.
        app.add_systems(
            Update,
            actions::add_pose_actions_to_set,
        );
        // After session is created, spawn pose space entities
        app.add_systems(
            bevy_mod_xr::session::XrSessionCreated,
            actions::create_pose_spaces,
        );
        // Re-send button/axis bindings + action set attachment at session creation.
        // The original messages from Startup expire before the user clicks "Start VR".
        // OxrSendActionBindings runs right before bind_actions in the session creation flow.
        app.add_systems(
            bevy_mod_openxr::action_binding::OxrSendActionBindings,
            (
                actions::resend_button_bindings,
                actions::resend_pose_bindings,
                actions::resend_action_set_attachment,
            ),
        );

        // Input systems — ordered after action state sync
        app.add_systems(
            Update,
            (
                input::sync_controller_state,
                input::sync_hand_tracking,
            )
                .run_if(resource_exists::<VrModeActive>),
        );

        // Haptic feedback — after controller state
        app.add_systems(
            Update,
            haptics::process_haptic_events
                .after(input::sync_controller_state)
                .run_if(resource_exists::<VrModeActive>),
        );

        // Interaction systems
        app.add_systems(
            Update,
            (
                interaction::teleport_system,
                interaction::smooth_locomotion_system,
                interaction::snap_turn_system,
                interaction::trigger_vertical_system,
                interaction::grab_system,
            )
                .chain()
                .after(input::sync_controller_state)
                .run_if(resource_exists::<VrModeActive>),
        );

        // Passthrough and blend mode
        app.add_systems(
            Update,
            (
                passthrough::update_passthrough,
                passthrough::update_blend_mode,
            )
                .run_if(resource_exists::<VrModeActive>),
        );

        // Audio bridge
        app.add_systems(
            Update,
            audio_bridge::sync_vr_head_to_audio_listener
                .run_if(resource_exists::<VrModeActive>),
        );
    }
}

/// Call this instead of `DefaultPlugins` when VR mode is requested.
///
/// Replaces `DefaultPlugins` with `add_xr_plugins()` from bevy_mod_openxr,
/// which sets up the OpenXR session, swapchain, and stereo rendering pipeline.
///
/// The `window` parameter is used as the desktop mirror window configuration.
/// Build the XR plugin group that replaces `DefaultPlugins`.
///
/// Returns a `PluginGroupBuilder` that should be added via `app.add_plugins()`
/// instead of `DefaultPlugins`. The caller passes `DefaultPlugins` in and gets
/// back a modified plugin group with XR support.
pub fn build_xr_plugins(window: bevy::window::Window) -> bevy::app::PluginGroupBuilder {
    use bevy_mod_openxr::add_xr_plugins;
    use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;

    // Request extensions based on config
    let mut exts = bevy_mod_openxr::exts::OxrExtensions::default();
    exts.enable_hand_tracking();
    exts.enable_fb_passthrough();
    exts.raw_mut().fb_display_refresh_rate = true;

    // Force NoVsync on the desktop mirror window so it doesn't throttle
    // the VR frame rate to the monitor's refresh rate.
    let mut vr_window = window;
    vr_window.present_mode = bevy::window::PresentMode::AutoNoVsync;

    // Disable PipelinedRenderingPlugin for lower tracking latency —
    // view poses are located right before rendering instead of one frame ahead.
    // add_xr_plugins() replaces DefaultPlugins entirely — it includes
    // WindowPlugin, RenderPlugin, AssetPlugin, etc. configured for XR.
    // We disable auto_handle so the editor controls session start/stop manually.
    add_xr_plugins(bevy::DefaultPlugins.build().disable::<PipelinedRenderingPlugin>())
        .set(bevy_mod_xr::session::XrSessionPlugin { auto_handle: false })
        .set(bevy::window::WindowPlugin {
            primary_window: Some(vr_window),
            ..default()
        })
        .set(bevy::asset::AssetPlugin {
            unapproved_path_mode: bevy::asset::UnapprovedPathMode::Allow,
            ..default()
        })
        .set(bevy_mod_openxr::init::OxrInitPlugin {
            exts,
            ..default()
        })
}

/// Finalize VR mode on the app after plugins are added.
///
/// Call this after `app.add_plugins(build_xr_plugins(...))`.
pub fn finalize_xr(app: &mut App) {
    app.add_plugins(XRUtilsActionsPlugin);
    app.insert_resource(VrModeActive);
    resources::vr_success("OpenXR plugins initialized — VR mode active");
}

/// Re-exports of key types from bevy_mod_openxr and bevy_mod_xr
/// so consumers don't need direct dependencies on those crates.
pub mod reexports {
    // Session & state
    pub use bevy_mod_xr::session::{XrState, XrTrackingRoot, XrTracker, XrSessionCreated, XrCreateSessionMessage, XrRequestExitMessage, XrStateChanged, XrBeginSessionMessage, XrEndSessionMessage, XrDestroySessionMessage};
    // Spaces
    pub use bevy_mod_xr::spaces::{XrSpace, XrReferenceSpace, XrPrimaryReferenceSpace};
    pub use bevy_mod_xr::spaces::{XrSpaceLocationFlags, XrSpaceVelocityFlags, XrVelocity};
    // Hands
    pub use bevy_mod_xr::hands::{HandBone, HandSide, LeftHand, RightHand};
    pub use bevy_mod_xr::hands::{XrHandBoneEntities, XrHandBoneRadius, HAND_JOINT_COUNT};
    // Camera
    pub use bevy_mod_xr::camera::{XrCamera, XrProjection};
    // OpenXR resources
    pub use bevy_mod_openxr::resources::{OxrInstance, OxrFrameState, OxrViews};
    pub use bevy_mod_openxr::session::OxrSession;
    // Extensions
    pub use bevy_mod_openxr::exts::{OxrEnabledExtensions, OxrExtensions};
    // Environment
    pub use bevy_mod_openxr::environment_blend_mode::OxrEnvironmentBlendModes;
    // Passthrough
    pub use bevy_mod_openxr::resources::{OxrPassthrough, OxrPassthroughLayerFB};
    // Helper traits
    pub use bevy_mod_openxr::helper_traits::*;
}
