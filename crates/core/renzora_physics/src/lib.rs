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
        info!("[runtime] PhysicsPlugin");
        let start_paused = cfg!(feature = "editor");

        app.register_type::<PhysicsBodyData>()
            .register_type::<PhysicsBodyType>()
            .register_type::<CollisionShapeData>()
            .register_type::<CollisionShapeType>();

        #[cfg(feature = "avian")]
        app.add_plugins(backend::avian::AvianBackendPlugin { start_paused });

        #[cfg(feature = "rapier")]
        app.add_plugins(backend::rapier::RapierBackendPlugin { start_paused });

        app.add_systems(Update, (auto_init_physics, sync_physics_data));
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
        commands.entity(entity).try_insert(RuntimePhysics);
    }
}

/// Automatically initialize backend components for entities that have physics data
/// components but haven't been wired up yet (no `RuntimePhysics` marker).
fn auto_init_physics(
    mut commands: Commands,
    new_bodies: Query<
        (Entity, Option<&PhysicsBodyData>, Option<&CollisionShapeData>, Option<&Name>),
        (Without<RuntimePhysics>, Or<(With<PhysicsBodyData>, With<CollisionShapeData>)>),
    >,
) {
    for (entity, body, shape, name) in &new_bodies {
        let label = name.map(|n| n.as_str()).unwrap_or("unnamed");
        info!("[Physics] Initialized physics on '{}' {:?} (body={}, shape={})",
            label, entity, body.is_some(), shape.is_some());
        renzora_core::console_log::console_info("Physics",
            format!("Initialized physics on '{}' (body={}, shape={})", label, body.is_some(), shape.is_some()));
        if let Some(b) = body {
            spawn_physics_body(&mut commands, entity, b);
        }
        if let Some(s) = shape {
            spawn_collision_shape(&mut commands, entity, s);
        }
        commands.entity(entity).try_insert(RuntimePhysics);
    }
}

/// Re-apply backend components when PhysicsBodyData or CollisionShapeData change at runtime.
fn sync_physics_data(
    mut commands: Commands,
    changed_bodies: Query<(Entity, &PhysicsBodyData), (With<RuntimePhysics>, Changed<PhysicsBodyData>)>,
    changed_shapes: Query<(Entity, &CollisionShapeData), (With<RuntimePhysics>, Changed<CollisionShapeData>)>,
) {
    for (entity, body_data) in &changed_bodies {
        spawn_physics_body(&mut commands, entity, body_data);
    }
    for (entity, shape_data) in &changed_shapes {
        spawn_collision_shape(&mut commands, entity, shape_data);
    }
}

/// Unpause the physics simulation.
pub fn unpause(world: &mut World) {
    info!("[Physics] Unpausing physics simulation");
    renzora_core::console_log::console_info("Physics", "Physics simulation unpaused");
    #[cfg(feature = "avian")]
    {
        use avian3d::schedule::PhysicsTime;
        if let Some(mut time) = world.get_resource_mut::<Time<avian3d::prelude::Physics>>() {
            time.unpause();
        }
    }
    #[cfg(feature = "rapier")]
    {
        // TODO: rapier unpause
    }
}

/// Pause the physics simulation.
pub fn pause(world: &mut World) {
    info!("[Physics] Pausing physics simulation");
    renzora_core::console_log::console_info("Physics", "Physics simulation paused");
    #[cfg(feature = "avian")]
    {
        use avian3d::schedule::PhysicsTime;
        if let Some(mut time) = world.get_resource_mut::<Time<avian3d::prelude::Physics>>() {
            time.pause();
        }
    }
    #[cfg(feature = "rapier")]
    {
        // TODO: rapier pause
    }
}
