//! Light rehydration.
//!
//! Bevy's `write_to_world` inserts components via reflection, which does not
//! run the required-components machinery — so a light deserializes fine and
//! then arrives missing the `Transform` / `Visibility` that `#[require(...)]`
//! would have supplied. Without them the light renders at the world origin, or
//! not at all.

use bevy::prelude::*;
use renzora_lighting::Sun;

/// Rehydrate sun entities — syncs `DirectionalLight` + `Transform` from `Sun` on newly added entities.
pub fn rehydrate_suns(mut query: Query<(&Sun, &mut DirectionalLight, &mut Transform), Added<Sun>>) {
    for (sun, mut light, mut transform) in &mut query {
        light.color = Color::srgb(sun.color.x, sun.color.y, sun.color.z);
        light.illuminance = sun.illuminance;
        light.shadow_maps_enabled = sun.shadows_enabled;
        *transform =
            Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, sun.direction()));
    }
}

/// Backfill required components on light entities loaded from a scene.
///
/// Bevy's `DynamicScene::write_to_world` inserts components via reflection,
/// which doesn't run the required-components machinery. Lights deserialize
/// fine but often arrive missing `Transform`, `Visibility`, etc — the
/// dependent components that `#[require(...)]` would normally auto-insert
/// when the light was first added via Commands. Without those, the light
/// either doesn't render or renders at world origin.
///
/// This system runs every frame and patches in any missing companions on
/// freshly-loaded light entities. Cheap when there are no lights to fix
/// (the `Without<...>` filters keep the query empty).
pub fn rehydrate_lights(
    mut commands: Commands,
    needs_transform: Query<
        Entity,
        (
            Or<(
                With<bevy::light::PointLight>,
                With<bevy::light::SpotLight>,
                With<bevy::light::DirectionalLight>,
            )>,
            Without<Transform>,
        ),
    >,
    needs_visibility: Query<
        Entity,
        (
            Or<(
                With<bevy::light::PointLight>,
                With<bevy::light::SpotLight>,
                With<bevy::light::DirectionalLight>,
            )>,
            Without<Visibility>,
        ),
    >,
) {
    for entity in &needs_transform {
        commands.entity(entity).try_insert(Transform::default());
    }
    for entity in &needs_visibility {
        commands.entity(entity).try_insert(Visibility::default());
    }
}
