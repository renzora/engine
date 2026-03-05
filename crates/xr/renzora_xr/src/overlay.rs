//! Overlay support for VR
//!
//! Provides configuration helpers for OpenXR overlay mode,
//! which allows the application to render as an overlay on top
//! of another VR application.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use crate::resources::vr_info;

/// Overlay configuration resource.
///
/// When enabled, the application runs as an overlay layer rather than
/// the primary VR application. This is an advanced feature.
#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct VrOverlayConfig {
    /// Whether overlay mode is enabled
    pub enabled: bool,
    /// Overlay sort order (higher = rendered on top)
    pub sort_order: i32,
}

impl Default for VrOverlayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            sort_order: 0,
        }
    }
}

/// Configure overlay mode for the application.
///
/// Call this during app setup before the XR session is created.
/// Overlay mode allows the app to render as a layer on top of
/// another VR application.
pub fn configure_overlay(app: &mut App) {
    app.init_resource::<VrOverlayConfig>();
    vr_info("VR overlay support configured");
}
