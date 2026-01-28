//! Physics component spawner
//!
//! Converts PhysicsBodyData and CollisionShapeData to Avian 3D components.
//! Used by both editor play mode and runtime.

#[cfg(feature = "physics")]
use avian3d::prelude::*;
use bevy::prelude::*;

use super::components::{CollisionShapeData, CollisionShapeType, PhysicsBodyData, PhysicsBodyType};

/// Spawn physics components on an entity based on PhysicsBodyData
///
/// This converts our serializable PhysicsBodyData to Avian's RigidBody and related components.
#[cfg(feature = "physics")]
pub fn spawn_physics_body(commands: &mut Commands, entity: Entity, body_data: &PhysicsBodyData) {
    let rigid_body = match body_data.body_type {
        PhysicsBodyType::RigidBody => RigidBody::Dynamic,
        PhysicsBodyType::StaticBody => RigidBody::Static,
        PhysicsBodyType::KinematicBody => RigidBody::Kinematic,
    };

    // Build locked axes from individual axis locks using Avian's method-chaining API
    let mut locked = LockedAxes::new();
    if body_data.lock_rotation_x {
        locked = locked.lock_rotation_x();
    }
    if body_data.lock_rotation_y {
        locked = locked.lock_rotation_y();
    }
    if body_data.lock_rotation_z {
        locked = locked.lock_rotation_z();
    }
    if body_data.lock_translation_x {
        locked = locked.lock_translation_x();
    }
    if body_data.lock_translation_y {
        locked = locked.lock_translation_y();
    }
    if body_data.lock_translation_z {
        locked = locked.lock_translation_z();
    }

    commands.entity(entity).insert((
        rigid_body,
        Mass(body_data.mass),
        GravityScale(body_data.gravity_scale),
        LinearDamping(body_data.linear_damping),
        AngularDamping(body_data.angular_damping),
        locked,
    ));
}

/// Spawn collider components on an entity based on CollisionShapeData
///
/// This converts our serializable CollisionShapeData to Avian's Collider and related components.
#[cfg(feature = "physics")]
pub fn spawn_collision_shape(
    commands: &mut Commands,
    entity: Entity,
    shape_data: &CollisionShapeData,
) {
    let collider = match shape_data.shape_type {
        CollisionShapeType::Box => {
            // Avian cuboid takes half-extents
            Collider::cuboid(
                shape_data.half_extents.x * 2.0,
                shape_data.half_extents.y * 2.0,
                shape_data.half_extents.z * 2.0,
            )
        }
        CollisionShapeType::Sphere => Collider::sphere(shape_data.radius),
        CollisionShapeType::Capsule => {
            // Avian capsule takes radius and total height (not half-height)
            Collider::capsule(shape_data.radius, shape_data.half_height * 2.0)
        }
        CollisionShapeType::Cylinder => {
            // Avian cylinder takes radius and total height
            Collider::cylinder(shape_data.radius, shape_data.half_height * 2.0)
        }
    };

    // Add collider offset if non-zero
    let mut entity_commands = commands.entity(entity);

    if shape_data.offset != Vec3::ZERO {
        // Use ColliderTransform for offset
        entity_commands.insert((
            collider,
            ColliderTransform::from(Transform::from_translation(shape_data.offset)),
            Friction::new(shape_data.friction),
            Restitution::new(shape_data.restitution),
        ));
    } else {
        entity_commands.insert((
            collider,
            Friction::new(shape_data.friction),
            Restitution::new(shape_data.restitution),
        ));
    }

    // Add sensor marker if this is a trigger
    if shape_data.is_sensor {
        entity_commands.insert(Sensor);
    }
}

/// Remove physics components from an entity
#[cfg(feature = "physics")]
pub fn despawn_physics_components(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<RigidBody>()
        .remove::<Mass>()
        .remove::<GravityScale>()
        .remove::<LinearDamping>()
        .remove::<AngularDamping>()
        .remove::<LockedAxes>()
        .remove::<Collider>()
        .remove::<ColliderTransform>()
        .remove::<Friction>()
        .remove::<Restitution>()
        .remove::<Sensor>()
        .remove::<LinearVelocity>()
        .remove::<AngularVelocity>()
        // Note: Avian uses ConstantForce/ConstantTorque for persistent forces
        // One-time forces are applied via the Forces QueryData helper
        .remove::<ConstantForce>()
        .remove::<ConstantTorque>();
}

// Stub implementations when physics is disabled
#[cfg(not(feature = "physics"))]
pub fn spawn_physics_body(_commands: &mut Commands, _entity: Entity, _body_data: &PhysicsBodyData) {
    // Physics disabled - no-op
}

#[cfg(not(feature = "physics"))]
pub fn spawn_collision_shape(
    _commands: &mut Commands,
    _entity: Entity,
    _shape_data: &CollisionShapeData,
) {
    // Physics disabled - no-op
}

#[cfg(not(feature = "physics"))]
pub fn despawn_physics_components(_commands: &mut Commands, _entity: Entity) {
    // Physics disabled - no-op
}

/// Marker component to track entities that have physics components spawned
/// Used to know which entities need physics cleanup when exiting play mode
#[derive(Component)]
pub struct RuntimePhysics;

/// Spawn all physics components for an entity that has PhysicsBodyData and/or CollisionShapeData
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

    // Mark entity as having runtime physics so we can clean up later
    if has_physics {
        commands.entity(entity).insert(RuntimePhysics);
    }
}
