//! ABI-stable types for plugin communication.
//!
//! Re-exports types from editor_plugin_api and provides Bevy conversion utilities.

// Re-export all types from the shared crate
pub use editor_plugin_api::abi::*;

use bevy::prelude::{Entity, Quat, Transform, Vec3};

/// Convert a Bevy Entity to EntityId
pub fn entity_to_id(entity: Entity) -> EntityId {
    EntityId(entity.to_bits())
}

/// Convert an EntityId to Bevy Entity (returns None if invalid)
pub fn id_to_entity(id: EntityId) -> Option<Entity> {
    if id.is_valid() {
        Some(Entity::from_bits(id.0))
    } else {
        None
    }
}

/// Convert a Bevy Transform to PluginTransform
pub fn transform_to_plugin(t: Transform) -> PluginTransform {
    PluginTransform {
        translation: t.translation.to_array(),
        rotation: t.rotation.to_array(),
        scale: t.scale.to_array(),
    }
}

/// Convert a PluginTransform to Bevy Transform
pub fn plugin_to_transform(t: PluginTransform) -> Transform {
    Transform {
        translation: Vec3::from_array(t.translation),
        rotation: Quat::from_array(t.rotation),
        scale: Vec3::from_array(t.scale),
    }
}

/// Extension trait for EntityId to add Bevy conversion methods
pub trait EntityIdExt {
    fn from_bevy(entity: Entity) -> Self;
    fn to_bevy(&self) -> Option<Entity>;
}

impl EntityIdExt for EntityId {
    fn from_bevy(entity: Entity) -> Self {
        entity_to_id(entity)
    }

    fn to_bevy(&self) -> Option<Entity> {
        id_to_entity(*self)
    }
}

/// Extension trait for PluginTransform to add Bevy conversion methods
pub trait PluginTransformExt {
    fn from_bevy(transform: Transform) -> Self;
    fn to_bevy(&self) -> Transform;
}

impl PluginTransformExt for PluginTransform {
    fn from_bevy(transform: Transform) -> Self {
        transform_to_plugin(transform)
    }

    fn to_bevy(&self) -> Transform {
        plugin_to_transform(*self)
    }
}
