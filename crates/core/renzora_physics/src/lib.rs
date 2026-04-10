pub mod data;
pub mod properties;
pub mod backend;
pub mod character_controller;
pub mod character_controller_systems;
#[cfg(feature = "editor")]
pub mod inspector;

pub use data::*;
pub use properties::*;
pub use character_controller::*;

use bevy::prelude::*;
use renzora_core::PlayModeState;

/// Run condition: true when NOT in editing mode (i.e. playing, scripts-only, or no PlayModeState resource).
fn not_editing(play_mode: Option<Res<PlayModeState>>) -> bool {
    play_mode.map_or(true, |pm| !pm.is_editing())
}

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
            .register_type::<CollisionShapeType>()
            .register_type::<CharacterControllerData>()
            .register_type::<CharacterControllerInput>();

        #[cfg(feature = "avian")]
        app.add_plugins(backend::avian::AvianBackendPlugin { start_paused });

        #[cfg(feature = "rapier")]
        app.add_plugins(backend::rapier::RapierBackendPlugin { start_paused });

        app.add_systems(Update, (auto_init_physics, sync_physics_data));

        // Character controller systems — only active during play mode.
        app.add_systems(PreUpdate, (
            character_controller_systems::clear_character_input,
            character_controller_systems::auto_input_from_actions,
        )
            .chain()
            .run_if(not_editing));
        app.add_systems(Update,
            character_controller_systems::auto_init_character_controller,
        );
        app.add_systems(Update,
            character_controller_systems::process_character_commands
                .run_if(not_editing),
        );

        #[cfg(feature = "avian")]
        {
            app.add_systems(
                Update,
                (
                    backend::avian_character::character_ground_check,
                    backend::avian_character::character_movement,
                    backend::avian_character::character_apply_velocity,
                )
                    .chain()
                    .after(character_controller_systems::process_character_commands)
                    .run_if(not_editing),
            );
        }

        // Listen for editor pause/unpause events (decoupled from renzora_editor_framework)
        app.add_observer(on_pause_physics)
           .add_observer(on_unpause_physics);

        #[cfg(feature = "avian")]
        app.add_systems(PostUpdate, clear_avian_forces.run_if(not_editing));

        // Listen for script actions (apply_force, apply_impulse, set_velocity)
        app.add_observer(handle_physics_script_actions);
    }
}

/// System to clear avian forces each frame (since we use ConstantForce for one-time pushes).
#[cfg(feature = "avian")]
fn clear_avian_forces(mut commands: Commands, query: Query<Entity, With<avian3d::prelude::ConstantForce>>) {
    for entity in &query {
        commands.entity(entity).remove::<avian3d::prelude::ConstantForce>();
    }
}

/// Observer: handle physics commands (apply_force, apply_impulse, set_velocity) from scripts.
fn handle_physics_script_actions(
    trigger: On<renzora_core::ScriptAction>,
    mut commands: Commands,
) {
    let action = trigger.event();
    let name = action.name.as_str();
    if !matches!(name, "apply_force" | "apply_impulse" | "set_velocity") {
        return;
    }

    use renzora_core::ScriptActionValue;
    let x = match action.args.get("x") { Some(ScriptActionValue::Float(v)) => *v, _ => 0.0 };
    let y = match action.args.get("y") { Some(ScriptActionValue::Float(v)) => *v, _ => 0.0 };
    let z = match action.args.get("z") { Some(ScriptActionValue::Float(v)) => *v, _ => 0.0 };
    let vec = Vec3::new(x, y, z);

    // Default to the entity that triggered the action, or use target ID if provided
    let target = if let Some(Some(ScriptActionValue::Int(id))) = action.args.get("entity_id").map(Some) {
        Entity::from_bits(*id as u64)
    } else {
        action.entity
    };

    match name {
        "apply_force" => {
            #[cfg(feature = "avian")]
            commands.entity(target).insert(avian3d::prelude::ConstantForce(vec));
            #[cfg(feature = "rapier")]
            { /* TODO: rapier apply_force */ }
        }
        "apply_impulse" => {
            #[cfg(feature = "avian")]
            {
                // Avian 0.6.1 doesn't have a built-in one-shot impulse component in prelude.
                // We'll apply it by inserting LinearVelocity which avian's solver will integrate.
                // This is a simplified impulse. For a true additive impulse we'd need a solver hook.
                commands.entity(target).insert(avian3d::prelude::LinearVelocity(vec));
            }
            #[cfg(feature = "rapier")]
            { /* TODO: rapier apply_impulse */ }
        }
        "set_velocity" => {
            #[cfg(feature = "avian")]
            commands.entity(target).insert(avian3d::prelude::LinearVelocity(vec));
            #[cfg(feature = "rapier")]
            { /* TODO: rapier set_velocity */ }
        }
        _ => {}
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

/// Observer: pause physics when the editor sends `PausePhysics`.
fn on_pause_physics(_trigger: On<renzora_core::PausePhysics>, mut commands: Commands) {
    commands.queue(|world: &mut World| pause(world));
}

/// Observer: unpause physics when the editor sends `UnpausePhysics`.
fn on_unpause_physics(_trigger: On<renzora_core::UnpausePhysics>, mut commands: Commands) {
    commands.queue(|world: &mut World| unpause(world));
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
