//! Passthrough and blend mode management for mixed reality
//!
//! Toggles OpenXR passthrough layers and environment blend modes
//! based on VrConfig settings.

use bevy::prelude::*;
use bevy_mod_openxr::resources::{OxrPassthrough, OxrPassthroughLayerFB};
use bevy_mod_openxr::environment_blend_mode::OxrEnvironmentBlendModes;

use crate::{BlendMode, VrConfig};
use crate::resources::{vr_info, vr_warn};

/// System: toggle passthrough on/off based on VrConfig.passthrough_enabled.
///
/// Uses `Local<bool>` to track previous state and only call start/pause
/// on transitions.
pub fn update_passthrough(
    config: Res<VrConfig>,
    passthrough: Option<ResMut<OxrPassthrough>>,
    passthrough_layer: Option<ResMut<OxrPassthroughLayerFB>>,
    mut prev_enabled: Local<bool>,
) {
    if config.passthrough_enabled == *prev_enabled {
        return;
    }
    *prev_enabled = config.passthrough_enabled;

    if config.passthrough_enabled {
        // Start passthrough
        if let Some(pt) = passthrough {
            if let Err(e) = pt.start() {
                vr_warn(format!("Failed to start passthrough: {e}"));
                return;
            }
        }
        if let Some(layer) = passthrough_layer {
            if let Err(e) = layer.resume() {
                vr_warn(format!("Failed to resume passthrough layer: {e}"));
            }
        }
        vr_info("Passthrough enabled");
    } else {
        // Stop passthrough
        if let Some(layer) = passthrough_layer {
            let _ = layer.pause();
        }
        if let Some(pt) = passthrough {
            let _ = pt.pause();
        }
        vr_info("Passthrough disabled");
    }
}

/// System: set environment blend mode from VrConfig.blend_mode.
pub fn update_blend_mode(
    config: Res<VrConfig>,
    blend_modes: Option<ResMut<OxrEnvironmentBlendModes>>,
    mut prev_mode: Local<Option<BlendMode>>,
) {
    if Some(config.blend_mode) == *prev_mode {
        return;
    }
    *prev_mode = Some(config.blend_mode);

    let Some(mut modes) = blend_modes else { return };

    let oxr_mode = match config.blend_mode {
        BlendMode::Opaque => openxr::EnvironmentBlendMode::OPAQUE,
        BlendMode::Additive => openxr::EnvironmentBlendMode::ADDITIVE,
        BlendMode::AlphaBlend => openxr::EnvironmentBlendMode::ALPHA_BLEND,
    };

    modes.set_blend_mode(oxr_mode);
    vr_info(format!("Blend mode set to {:?}", config.blend_mode));
}
