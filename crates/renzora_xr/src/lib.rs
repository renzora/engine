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
mod eyes;
mod input;
mod locomotion;
mod rig;
mod session;

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
/// Whether an OpenXR runtime is reachable right now — the cheap boot-time
/// probe that decides if the editor boots XR-capable. `Entry::load()` finds
/// the OpenXR loader; enumerating extensions actually calls into the active
/// runtime, so it fails when no runtime is registered/running.
pub fn runtime_available() -> bool {
    let Ok(entry) = (unsafe { openxr::Entry::load() }) else {
        return false;
    };
    entry.enumerate_extensions().is_ok()
}

/// True when this app booted in `--vr` game mode (session auto-starts, desktop
/// mirror on) as opposed to an XR-capable editor session (session on demand,
/// the viewport panel is the mirror).
#[derive(Resource, Clone, Copy)]
pub struct XrBootMode {
    pub game: bool,
}

pub fn xr_plugins(base: PluginGroupBuilder, auto_start: bool) -> PluginGroupBuilder {
    let mut exts = bevy_mod_openxr::exts::OxrExtensions::default();
    // Hand tracking + passthrough are enabled at the OpenXR-instance level so
    // runtimes that support them expose the data; consuming hand joints is
    // future work. Display-refresh-rate lets Quest-class devices run 90/120Hz.
    exts.enable_hand_tracking();
    exts.enable_fb_passthrough();
    exts.raw_mut().fb_display_refresh_rate = true;

    bevy_mod_openxr::add_xr_plugins(base)
        .set(bevy_mod_xr::session::XrSessionPlugin {
            auto_handle: auto_start,
        })
        // Eye cameras are spawned by eyes.rs with the engine's full camera
        // decoration (bind-group layouts lock at first render, so the backend's
        // bare spawn can never gain atmosphere/IBL afterwards). The backend
        // still registers the swapchain texture views and adopts our cameras.
        .set(bevy_mod_openxr::render::OxrRenderPlugin {
            spawn_cameras: false,
            default_wait_frame: true,
        })
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

/// The Renzora VR layer: input, locomotion, rig visuals, environment sync,
/// and on-demand session control. `auto_start: true` for `--vr` game boots
/// (headset lights up immediately, desktop mirror on); `false` for XR-capable
/// editor sessions (session follows the "VR Headset" play target).
pub struct XrPlugin {
    pub auto_start: bool,
}

impl Plugin for XrPlugin {
    fn build(&self, app: &mut App) {
        info!(
            "[runtime] XrPlugin (OpenXR, {})",
            if self.auto_start { "game mode" } else { "editor on-demand" }
        );

        app.insert_resource(XrBootMode {
            game: self.auto_start,
        });
        app.init_resource::<VrConfig>().init_resource::<VrInput>();

        // Marker-driven pose tracking (grip poses, head view) used by the
        // controller visuals and the desktop mirror camera.
        app.add_plugins(bevy_xr_utils::tracking_utils::TrackingUtilitiesPlugin);
        app.add_plugins(bevy_xr_utils::actions::XRUtilsActionsPlugin);

        eyes::register(app);
        input::register(app);
        locomotion::register(app);
        rig::register(app);
        environment::register(app);
        session::register(app, self.auto_start);

        // Session-state breadcrumbs: "window opened but headset is dark" is
        // otherwise undiagnosable for users — the state transitions (or the
        // lack of any) say exactly where OpenXR got stuck.
        app.add_systems(Update, log_session_state);
    }
}

/// Log every XR session-state transition, plus a setup hint when the runtime
/// reports no XR system at all (headset not in Link mode / wrong default
/// OpenXR runtime — the most common first-run failure).
fn log_session_state(state: Res<XrState>, mut last: Local<Option<XrState>>) {
    if *last == Some(*state) {
        return;
    }
    *last = Some(*state);
    info!("[XR] session state: {state:?}");
    if matches!(*state, XrState::Unavailable) {
        warn!(
            "[XR] no headset available. Check: headset connected and in Link \
             mode (Quest: enable Quest Link / Air Link in the headset), and \
             the matching runtime set as the system's default OpenXR runtime \
             (Meta Quest Link app -> Settings -> General -> 'Set Meta Quest \
             Link as active OpenXR runtime', or SteamVR -> Settings -> OpenXR)."
        );
    }
}
