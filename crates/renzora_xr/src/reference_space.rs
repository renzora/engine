//! Reference space configuration for seated/standing modes
//!
//! Switches between STAGE (standing) and LOCAL (seated) reference spaces
//! based on VrConfig.seated_mode.

use bevy::prelude::*;
use bevy_mod_openxr::session::OxrSession;
use bevy_mod_xr::spaces::XrPrimaryReferenceSpace;

use crate::VrConfig;
use crate::resources::{vr_info, vr_warn};

/// System: configure reference space based on seated_mode setting.
///
/// Runs when the XR session is created. If seated_mode is true,
/// creates a LOCAL reference space (seated, head-relative origin).
/// If standing, the default STAGE space from OxrReferenceSpacePlugin is correct.
pub fn configure_reference_space(
    config: Res<VrConfig>,
    session: Option<Res<OxrSession>>,
    mut commands: Commands,
) {
    if !config.seated_mode {
        // Standing mode uses the default STAGE reference space
        return;
    }

    let Some(session) = session else { return };

    // Create LOCAL reference space for seated mode
    // bevy_oxr's create_reference_space takes Isometry3d, returns XrReferenceSpace
    match session.create_reference_space(
        openxr::ReferenceSpaceType::LOCAL,
        bevy::math::Isometry3d::IDENTITY,
    ) {
        Ok(space) => {
            // Insert as the primary reference space resource (overrides the default STAGE)
            commands.insert_resource(XrPrimaryReferenceSpace(space));

            vr_info("Seated mode: switched to LOCAL reference space");
        }
        Err(e) => {
            vr_warn(format!("Failed to create LOCAL reference space: {e}"));
        }
    }
}
