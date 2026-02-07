//! Light data sync system
//!
//! Individual light component definitions are in their own files:
//! point_light.rs, directional_light.rs, spot_light.rs, solari_lighting.rs

use bevy::prelude::*;

use crate::shared::{DirectionalLightData, PointLightData, SpotLightData, SunData};

/// Syncs wrapper data types to Bevy's built-in light components.
/// Should be registered as a Bevy system so that inspector edits to the
/// data components are automatically propagated to the actual lights.
pub fn sync_light_data_to_bevy(
    mut point_lights: Query<(&PointLightData, &mut PointLight, Option<&crate::core::DisabledComponents>), Or<(Changed<PointLightData>, Changed<crate::core::DisabledComponents>)>>,
    mut dir_lights: Query<
        (&DirectionalLightData, &mut DirectionalLight, Option<&crate::core::DisabledComponents>),
        (Or<(Changed<DirectionalLightData>, Changed<crate::core::DisabledComponents>)>, Without<SunData>),
    >,
    mut spot_lights: Query<(&SpotLightData, &mut SpotLight, Option<&crate::core::DisabledComponents>), Or<(Changed<SpotLightData>, Changed<crate::core::DisabledComponents>)>>,
    mut sun_lights: Query<(&SunData, &mut DirectionalLight, &mut Transform, Option<&crate::core::DisabledComponents>), Or<(Changed<SunData>, Changed<crate::core::DisabledComponents>)>>,
) {
    for (data, mut light, dc) in &mut point_lights {
        let disabled = dc.map_or(false, |d| d.is_disabled("point_light"));
        light.color = Color::srgb(data.color.x, data.color.y, data.color.z);
        light.intensity = if disabled { 0.0 } else { data.intensity };
        light.range = data.range;
        light.radius = data.radius;
        light.shadows_enabled = if disabled { false } else { data.shadows_enabled };
    }

    for (data, mut light, dc) in &mut dir_lights {
        let disabled = dc.map_or(false, |d| d.is_disabled("directional_light"));
        light.color = Color::srgb(data.color.x, data.color.y, data.color.z);
        light.illuminance = if disabled { 0.0 } else { data.illuminance };
        light.shadows_enabled = if disabled { false } else { data.shadows_enabled };
    }

    for (data, mut light, dc) in &mut spot_lights {
        let disabled = dc.map_or(false, |d| d.is_disabled("spot_light"));
        light.color = Color::srgb(data.color.x, data.color.y, data.color.z);
        light.intensity = if disabled { 0.0 } else { data.intensity };
        light.range = data.range;
        light.radius = data.radius;
        light.inner_angle = data.inner_angle;
        light.outer_angle = data.outer_angle;
        light.shadows_enabled = if disabled { false } else { data.shadows_enabled };
    }

    for (data, mut light, mut transform, dc) in &mut sun_lights {
        let disabled = dc.map_or(false, |d| d.is_disabled("sun"));
        light.color = Color::srgb(data.color.x, data.color.y, data.color.z);
        light.illuminance = if disabled { 0.0 } else { data.illuminance };
        light.shadows_enabled = if disabled { false } else { data.shadows_enabled };
        // Orient the directional light based on azimuth/elevation
        let dir = data.direction();
        transform.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, dir);
    }
}
