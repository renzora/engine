//! OpenXR extension management and capability detection
//!
//! Provides `VrCapabilities` resource that reports which optional
//! XR features are available on the current runtime/hardware.

use bevy::prelude::*;
use bevy_mod_openxr::exts::OxrEnabledExtensions;
use serde::{Deserialize, Serialize};
use crate::resources::vr_info;

/// Detected VR hardware/runtime capabilities.
///
/// Populated after session creation by querying enabled OpenXR extensions.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct VrCapabilities {
    /// XR_EXT_hand_tracking is available
    pub hand_tracking_supported: bool,
    /// XR_FB_passthrough is available
    pub passthrough_supported: bool,
    /// XR_EXT_eye_gaze_interaction is available
    pub eye_tracking_supported: bool,
    /// Overlay rendering is supported
    pub overlay_supported: bool,
    /// XR_FB_foveation is available
    pub foveation_supported: bool,
    /// XR_MSFT_spatial_anchor is available
    pub spatial_anchors_supported: bool,
}

/// System: detect available capabilities from enabled extensions.
///
/// Runs once after session creation to populate `VrCapabilities`.
pub fn detect_capabilities(
    mut commands: Commands,
    enabled: Option<Res<OxrEnabledExtensions>>,
) {
    let Some(exts) = enabled else {
        commands.insert_resource(VrCapabilities::default());
        return;
    };

    let caps = VrCapabilities {
        hand_tracking_supported: exts.ext_hand_tracking,
        passthrough_supported: exts.fb_passthrough,
        eye_tracking_supported: exts.ext_eye_gaze_interaction,
        overlay_supported: exts.extx_overlay,
        foveation_supported: exts.fb_foveation || exts.fb_foveation_vulkan,
        spatial_anchors_supported: exts.msft_spatial_anchor,
    };

    vr_info(format!(
        "VR Capabilities detected — hand_tracking: {}, passthrough: {}, eye_tracking: {}, foveation: {}",
        caps.hand_tracking_supported,
        caps.passthrough_supported,
        caps.eye_tracking_supported,
        caps.foveation_supported,
    ));

    commands.insert_resource(caps);
}

/// Check if a named extension is available.
pub fn is_extension_available(exts: &OxrEnabledExtensions, name: &str) -> bool {
    match name {
        "hand_tracking" | "XR_EXT_hand_tracking" => exts.ext_hand_tracking,
        "passthrough" | "XR_FB_passthrough" => exts.fb_passthrough,
        "eye_tracking" | "XR_EXT_eye_gaze_interaction" => exts.ext_eye_gaze_interaction,
        "overlay" | "XR_EXTX_overlay" => exts.extx_overlay,
        "foveation" | "XR_FB_foveation" => exts.fb_foveation,
        "spatial_anchor" | "XR_MSFT_spatial_anchor" => exts.msft_spatial_anchor,
        _ => false,
    }
}
