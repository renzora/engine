//! Light component spawner
//!
//! Converts PointLightData, DirectionalLightData, and SpotLightData to Bevy light components.
//! Used by both editor play mode and runtime.

use bevy::prelude::*;

use super::components::{DirectionalLightData, PointLightData, SpotLightData};

/// Marker component to track entities that have light components spawned at runtime
#[derive(Component)]
pub struct RuntimeLight;

/// Spawn a PointLight component from PointLightData
pub fn spawn_point_light(commands: &mut Commands, entity: Entity, data: &PointLightData) {
    commands.entity(entity).insert(PointLight {
        color: Color::srgb(data.color.x, data.color.y, data.color.z),
        intensity: data.intensity,
        range: data.range,
        radius: data.radius,
        shadows_enabled: data.shadows_enabled,
        ..default()
    });
}

/// Spawn a DirectionalLight component from DirectionalLightData
pub fn spawn_directional_light(commands: &mut Commands, entity: Entity, data: &DirectionalLightData) {
    commands.entity(entity).insert(DirectionalLight {
        color: Color::srgb(data.color.x, data.color.y, data.color.z),
        illuminance: data.illuminance,
        shadows_enabled: data.shadows_enabled,
        ..default()
    });
}

/// Spawn a SpotLight component from SpotLightData
pub fn spawn_spot_light(commands: &mut Commands, entity: Entity, data: &SpotLightData) {
    commands.entity(entity).insert(SpotLight {
        color: Color::srgb(data.color.x, data.color.y, data.color.z),
        intensity: data.intensity,
        range: data.range,
        radius: data.radius,
        inner_angle: data.inner_angle,
        outer_angle: data.outer_angle,
        shadows_enabled: data.shadows_enabled,
        ..default()
    });
}

/// Spawn all light components for an entity based on its light data components
pub fn spawn_entity_lights(
    commands: &mut Commands,
    entity: Entity,
    point_light: Option<&PointLightData>,
    directional_light: Option<&DirectionalLightData>,
    spot_light: Option<&SpotLightData>,
) {
    let mut has_light = false;

    if let Some(data) = point_light {
        spawn_point_light(commands, entity, data);
        has_light = true;
    }

    if let Some(data) = directional_light {
        spawn_directional_light(commands, entity, data);
        has_light = true;
    }

    if let Some(data) = spot_light {
        spawn_spot_light(commands, entity, data);
        has_light = true;
    }

    // Mark entity as having runtime lights so we can track it
    if has_light {
        commands.entity(entity).insert(RuntimeLight);
    }
}

/// Remove light components from an entity
pub fn despawn_light_components(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<PointLight>()
        .remove::<DirectionalLight>()
        .remove::<SpotLight>()
        .remove::<RuntimeLight>();
}
