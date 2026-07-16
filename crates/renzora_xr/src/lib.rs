//! OpenXR/VR boot mode for the Renzora runtime.
//!
//! # Why a boot mode and not a plugin
//!
//! OpenXR owns graphics-device creation: the headset runtime dictates the
//! Vulkan instance/device the app must render with, so `bevy_mod_openxr`
//! replaces Bevy's `RenderPlugin` wholesale. That decision happens while the
//! `App` is being assembled — long before `plugins/` cdylibs are dlopen'd —
//! which is why VR cannot ship as a distribution plugin. Instead this crate is
//! statically linked into `renzora_runtime` (behind its `xr` feature) and the
//! binary opts in at launch with `--vr`:
//!
//! 1. [`xr_plugins`] wraps the runtime's `DefaultPlugins` build with the
//!    OpenXR plugin set (stereo swapchain cameras, session lifecycle, actions);
//! 2. [`XrPlugin`] layers the Renzora VR gameplay systems on top: controller
//!    input ([`VrInput`]), locomotion (smooth move + snap/smooth turn),
//!    controller visuals, a desktop mirror camera, and environment sync.
//!
//! The editor never runs in VR itself — its "VR Headset" play target launches
//! the runtime as a child process (`--no-editor --project <p> --vr`), the same
//! external-process flow as the "Window" play target. In the headset the scene
//! plays exactly as a shipped VR game would.
//!
//! `bevy_mod_openxr` spawns one `XrCamera` per eye targeting the swapchain,
//! and `XrSessionPlugin` auto-spawns the [`XrTrackingRoot`] the player rig
//! hangs off. Everything scene-side is ordinary ECS content — gaussian splat
//! clouds, particles, physics all render through the eye cameras like any
//! other view (the splat plugin auto-tags active 3D cameras, XR eyes
//! included).

use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

mod environment;
mod input;
mod locomotion;
mod rig;

pub use input::VrInput;

// Re-exported so runtime wiring and future tooling name XR types through one
// crate instead of reaching into the vendored stack.
pub use bevy_mod_xr::camera::XrCamera;
pub use bevy_mod_xr::session::{XrState, XrTrackingRoot};

/// Wrap the runtime's plugin-group build with the OpenXR stack.
///
/// Called by `renzora_runtime::add_xr_rendering` with the same
/// `DefaultPlugins`-derived builder the desktop path uses (minus
/// `PipelinedRenderingPlugin`, which the caller disables — pipelined rendering
/// adds a frame of pose latency because views would be located one frame
/// before they render). Returns the builder so the caller can keep layering
/// its window/asset/log configuration on top.
pub fn xr_plugins(base: PluginGroupBuilder) -> PluginGroupBuilder {
    let mut exts = bevy_mod_openxr::exts::OxrExtensions::default();
    // Hand tracking + passthrough are enabled at the OpenXR-instance level so
    // runtimes that support them expose the data; consuming hand joints is
    // future work. Display-refresh-rate lets Quest-class devices run 90/120Hz.
    exts.enable_hand_tracking();
    exts.enable_fb_passthrough();
    exts.raw_mut().fb_display_refresh_rate = true;

    bevy_mod_openxr::add_xr_plugins(base)
        .set(bevy_mod_xr::session::XrSessionPlugin { auto_handle: true })
        .set(bevy_mod_openxr::init::OxrInitPlugin {
            exts,
            ..Default::default()
        })
}

/// VR tuning knobs. Inserted with defaults at boot; a future settings page or
/// script API can rewrite them live.
#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct VrConfig {
    /// Smooth-locomotion speed, m/s.
    pub move_speed: f32,
    /// Degrees per snap turn.
    pub snap_turn_angle: f32,
    /// Seconds between snap turns while the stick is held.
    pub snap_turn_cooldown: f32,
    /// Stick magnitude below which input is ignored.
    pub thumbstick_deadzone: f32,
    /// Smooth (continuous) turning instead of snap turns.
    pub smooth_turn: bool,
    /// Render a head-tracked desktop mirror view into the flat window so
    /// spectators (and the developer) see what the player sees.
    pub desktop_mirror: bool,
    /// Show the procedural controller wands.
    pub controller_visuals: bool,
}

impl Default for VrConfig {
    fn default() -> Self {
        Self {
            move_speed: 4.0,
            snap_turn_angle: 45.0,
            snap_turn_cooldown: 0.3,
            thumbstick_deadzone: 0.2,
            smooth_turn: false,
            desktop_mirror: true,
            controller_visuals: true,
        }
    }
}

/// The Renzora VR layer: input, locomotion, rig visuals, environment sync.
/// Added by `add_xr_rendering` after the OpenXR plugin set.
#[derive(Default)]
pub struct XrPlugin;

impl Plugin for XrPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] XrPlugin (OpenXR VR mode)");

        app.init_resource::<VrConfig>().init_resource::<VrInput>();

        // Marker-driven pose tracking (grip poses, head view) used by the
        // controller visuals and the desktop mirror camera.
        app.add_plugins(bevy_xr_utils::tracking_utils::TrackingUtilitiesPlugin);
        app.add_plugins(bevy_xr_utils::actions::XRUtilsActionsPlugin);

        input::register(app);
        locomotion::register(app);
        rig::register(app);
        environment::register(app);
    }
}
