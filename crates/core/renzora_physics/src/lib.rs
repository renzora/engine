pub mod data;
pub mod properties;
pub mod backend;

pub use data::*;
pub use properties::*;

use bevy::prelude::*;

#[cfg(not(any(feature = "avian", feature = "rapier")))]
compile_error!("renzora_physics requires either the `avian` or `rapier` feature");

#[cfg(all(feature = "avian", feature = "rapier"))]
compile_error!("renzora_physics: enable only one of `avian` or `rapier`, not both");

/// Physics plugin that delegates to the selected backend.
///
/// Automatically starts paused when the `editor` feature is enabled,
/// and runs immediately in standalone/runtime mode.
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        let start_paused = cfg!(feature = "editor");

        app.register_type::<PhysicsBodyData>()
            .register_type::<PhysicsBodyType>()
            .register_type::<CollisionShapeData>()
            .register_type::<CollisionShapeType>();

        #[cfg(feature = "avian")]
        app.add_plugins(backend::avian::AvianBackendPlugin { start_paused });

        #[cfg(feature = "rapier")]
        app.add_plugins(backend::rapier::RapierBackendPlugin { start_paused });
    }
}

// Re-export backend functions under a unified API so callers don't need cfg guards.

/// Spawn physics body components on an entity.
pub fn spawn_physics_body(commands: &mut Commands, entity: Entity, body_data: &PhysicsBodyData) {
    #[cfg(feature = "avian")]
    backend::avian::spawn_physics_body(commands, entity, body_data);
    #[cfg(feature = "rapier")]
    backend::rapier::spawn_physics_body(commands, entity, body_data);
}

/// Spawn collider components on an entity.
pub fn spawn_collision_shape(commands: &mut Commands, entity: Entity, shape_data: &CollisionShapeData) {
    #[cfg(feature = "avian")]
    backend::avian::spawn_collision_shape(commands, entity, shape_data);
    #[cfg(feature = "rapier")]
    backend::rapier::spawn_collision_shape(commands, entity, shape_data);
}

/// Remove all physics components from an entity.
pub fn despawn_physics_components(commands: &mut Commands, entity: Entity) {
    #[cfg(feature = "avian")]
    backend::avian::despawn_physics_components(commands, entity);
    #[cfg(feature = "rapier")]
    backend::rapier::despawn_physics_components(commands, entity);
}

/// Spawn all physics components for an entity that has PhysicsBodyData and/or CollisionShapeData.
pub fn spawn_entity_physics(
    commands: &mut Commands,
    entity: Entity,
    body_data: Option<&PhysicsBodyData>,
    shape_data: Option<&CollisionShapeData>,
) {
    let mut has_physics = false;

    if let Some(body) = body_data {
        spawn_physics_body(commands, entity, body);
        has_physics = true;
    }

    if let Some(shape) = shape_data {
        spawn_collision_shape(commands, entity, shape);
        has_physics = true;
    }

    if has_physics {
        commands.entity(entity).insert(RuntimePhysics);
    }
}
