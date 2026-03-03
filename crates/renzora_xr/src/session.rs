//! OpenXR session lifecycle management
//!
//! Monitors the OpenXR session state via `XrState` and updates
//! `VrSessionState` accordingly. Emits `VrSessionStatusChanged` events
//! on transitions.

use bevy::prelude::*;
use bevy_mod_openxr::resources::{OxrFrameState, OxrInstance};
use bevy_mod_openxr::session::OxrSession;
use bevy_mod_xr::session::XrState;

use crate::resources::{VrSessionState, VrSessionStatusChanged, VrStatus, vr_info, vr_warn};

/// System: update VR session state from OpenXR session state and frame state.
///
/// Maps `XrState` to `VrStatus` and populates headset name from the runtime.
pub fn update_session_state(
    xr_state: Option<Res<XrState>>,
    frame_state: Option<Res<OxrFrameState>>,
    instance: Option<Res<OxrInstance>>,
    mut session_state: ResMut<VrSessionState>,
    mut status_events: MessageWriter<VrSessionStatusChanged>,
    mut headset_populated: Local<bool>,
) {
    // One-time diagnostic: log what we see on the first frame
    {
        static LOGGED_INIT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !LOGGED_INIT.swap(true, std::sync::atomic::Ordering::Relaxed) {
            let xr_desc = match xr_state.as_deref() {
                None => "None (resource not inserted)".to_string(),
                Some(s) => format!("{:?}", s),
            };
            vr_info(format!(
                "VR session diagnostics — XrState: {}, OxrInstance: {}, OxrFrameState: {}",
                xr_desc,
                instance.is_some(),
                frame_state.is_some(),
            ));
        }
    }

    let new_status = match xr_state.as_deref() {
        None => VrStatus::Disconnected,
        Some(state) => match state {
            XrState::Unavailable => {
                // Log once: init failed
                static LOGGED_UNAVAIL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
                if !LOGGED_UNAVAIL.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    vr_warn(
                        "XrState is Unavailable — OpenXR runtime failed to initialize. \
                         Check that your headset is connected and Meta Quest app is running.",
                    );
                }
                VrStatus::Disconnected
            }
            XrState::Available | XrState::Idle => VrStatus::Initializing,
            XrState::Ready => VrStatus::Ready,
            XrState::Running => {
                // Check frame state to distinguish Focused vs Visible
                if let Some(ref frame) = frame_state {
                    if frame.should_render {
                        VrStatus::Focused
                    } else {
                        VrStatus::Visible
                    }
                } else {
                    VrStatus::Visible
                }
            }
            XrState::Stopping => VrStatus::Stopping,
            XrState::Exiting { .. } => VrStatus::Stopped,
        },
    };

    // Populate headset name once from runtime properties
    if !*headset_populated {
        if let Some(ref inst) = instance {
            if let Ok(props) = inst.properties() {
                session_state.headset_name = props.runtime_name.clone();
                *headset_populated = true;
                vr_info(format!("VR headset: {}", session_state.headset_name));
            }
        }
    }

    // Sync refresh rate from the runtime's predicted display period
    if let Some(ref frame) = frame_state {
        let period_nanos = frame.predicted_display_period.as_nanos();
        if period_nanos > 0 {
            let hz = 1_000_000_000.0 / period_nanos as f64;
            session_state.refresh_rate = hz as f32;
        }
    }

    // Emit event and log on status change
    if session_state.status != new_status {
        let old = session_state.status;
        vr_info(format!("VR session status: {:?} -> {:?}", old, new_status));
        status_events.write(VrSessionStatusChanged {
            old_status: old,
            new_status,
        });
        session_state.status = new_status;
    }
}

/// System: enumerate available display refresh rates once the session is running.
///
/// Uses `XR_FB_display_refresh_rate` to query what rates the headset supports
/// (e.g. [72, 80, 90, 120] on Quest 3). Runs every frame but only queries once
/// (when `available_refresh_rates` is empty and session is running).
pub fn enumerate_refresh_rates(
    session: Option<Res<OxrSession>>,
    mut session_state: ResMut<VrSessionState>,
) {
    // Only query once
    if !session_state.available_refresh_rates.is_empty() {
        return;
    }
    let Some(session) = session else { return };

    match session.enumerate_display_refresh_rates() {
        Ok(mut rates) => {
            rates.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            vr_info(format!("Available refresh rates: {:?} Hz", rates));
            session_state.available_refresh_rates = rates;
        }
        Err(e) => {
            // Extension not supported or session not ready — not an error
            warn!("Could not enumerate refresh rates (XR_FB_display_refresh_rate may not be supported): {e}");
            // Put a sentinel so we don't retry every frame
            session_state.available_refresh_rates = vec![session_state.refresh_rate.max(72.0)];
        }
    }
}

/// System: apply a user-requested refresh rate change via `XR_FB_display_refresh_rate`.
///
/// Watches `VrSessionState.requested_refresh_rate` — when it changes to a non-zero
/// value, requests that rate from the runtime and resets the field.
pub fn apply_requested_refresh_rate(
    session: Option<Res<OxrSession>>,
    mut session_state: ResMut<VrSessionState>,
) {
    let requested = session_state.requested_refresh_rate;
    if requested <= 0.0 {
        return;
    }
    // Reset immediately so we don't retry every frame
    session_state.requested_refresh_rate = 0.0;

    let Some(session) = session else { return };

    match session.request_display_refresh_rate(requested) {
        Ok(()) => {
            vr_info(format!("Requested refresh rate: {:.0} Hz", requested));
        }
        Err(e) => {
            crate::resources::vr_warn(format!(
                "Failed to set refresh rate to {:.0} Hz: {e}", requested
            ));
        }
    }
}
