pub mod auto_fit;
pub mod backend;
pub mod data;
pub mod properties;
pub mod read_state;
pub mod script_extension;

/// When `active`, the editor enters "edit collider" mode for the selected entity:
/// the transform gizmo is hidden and (later) collider resize/move handles take over.
///
/// Lives in the lean crate (it stays `pub` here) because the editor-only
/// `renzora_gizmo` crate reads it as `renzora_physics::ColliderEditMode` to
/// drive the collider resize/move handles. The resource is *initialised* by
/// `renzora_physics_editor::PhysicsEditorPlugin`; the gizmo crate treats it as
/// optional so the lean runtime (no editor) simply never inserts it.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct ColliderEditMode {
    pub active: bool,
}

pub use data::*;
pub use properties::*;
pub use read_state::PhysicsReadState;

use bevy::prelude::*;
use renzora::PlayModeState;

/// Run condition: true when NOT in editing mode (i.e. playing, scripts-only, or no PlayModeState resource).
fn not_editing(play_mode: Option<Res<PlayModeState>>) -> bool {
    play_mode.is_none_or(|pm| !pm.is_editing())
}

#[cfg(not(any(feature = "avian", feature = "rapier")))]
compile_error!("renzora_physics requires either the `avian` or `rapier` feature");

#[cfg(all(feature = "avian", feature = "rapier"))]
compile_error!("renzora_physics: enable only one of `avian` or `rapier`, not both");

/// Physics plugin that delegates to the selected backend.
///
/// Runs the simulation immediately. In the editor the companion
/// `renzora_physics_editor::PhysicsEditorPlugin` adds a `Startup` system that
/// pauses the simulation (via [`pause`]) so the scene doesn't simulate until
/// the user hits play.
#[derive(Default)]
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] PhysicsPlugin");
        // The simulation runs immediately; the editor crate pauses it at
        // startup when present (see `PhysicsEditorPlugin`).
        let start_paused = false;

        app.register_type::<PhysicsBodyData>()
            .register_type::<PhysicsBodyType>()
            .register_type::<CollisionShapeData>()
            .register_type::<CollisionShapeType>()
            .register_type::<PhysicsReadState>();

        #[cfg(feature = "avian")]
        app.add_plugins(backend::avian::AvianBackendPlugin { start_paused });

        #[cfg(feature = "rapier")]
        app.add_plugins(backend::rapier::RapierBackendPlugin { start_paused });

        app.add_systems(Update, (auto_init_physics, sync_physics_data));
        app.add_systems(
            Update,
            (
                auto_fit::mark_new_collision_shapes,
                auto_fit::auto_fit_collision_shapes,
            )
                .chain(),
        );

        // Listen for editor pause/unpause events (decoupled from renzora_editor_framework)
        app.add_observer(on_pause_physics)
            .add_observer(on_unpause_physics);

        #[cfg(feature = "avian")]
        app.add_systems(PostUpdate, clear_avian_forces.run_if(not_editing));

        app.init_resource::<PendingKinematicSlides>();
        #[cfg(feature = "avian")]
        {
            app.init_resource::<ResolvedSlides>();
            app.add_systems(
                Update,
                (compute_kinematic_slides, apply_kinematic_slides)
                    .chain()
                    .run_if(not_editing),
            );
        }

        // Listen for script actions (apply_force, apply_impulse, set_velocity, kinematic_slide)
        app.add_observer(handle_physics_script_actions);

        // Per-entity read-state mirror + script extension.
        app.add_systems(Update, read_state::auto_init_physics_read_state);
        #[cfg(feature = "avian")]
        app.add_systems(Update, read_state::update_physics_read_state);

        // Register Lua/Rhai functions owned by the physics crate.
        {
            let mut extensions = app.world_mut().get_resource_or_insert_with(
                renzora_scripting::extension::ScriptExtensions::default,
            );
            extensions.register(script_extension::PhysicsScriptExtension);
        }
    }
}

/// One pending kinematic slide request.
#[derive(Clone, Copy, Debug)]
pub struct PendingSlide {
    pub entity: Entity,
    pub delta: Vec3,
    pub max_slope: f32,
}

/// Queue of slide requests produced by the `kinematic_slide` script action
/// and drained each frame by `drain_kinematic_slides`.
#[derive(Resource, Default)]
pub struct PendingKinematicSlides(pub Vec<PendingSlide>);

/// System: applies pending kinematic slides with full collision response.
/// Computed slide result waiting to be applied to `Position` + `Transform`.
/// Produced by `compute_kinematic_slides` and drained by `apply_kinematic_slides`
/// — split into two systems so the SpatialQuery reads don't conflict with the
/// `&mut Position` writes.
#[cfg(feature = "avian")]
#[derive(Resource, Default)]
struct ResolvedSlides(Vec<(Entity, Vec3, bool, Vec3)>);

#[cfg(feature = "avian")]
fn compute_kinematic_slides(
    mut queue: ResMut<PendingKinematicSlides>,
    mut resolved: ResMut<ResolvedSlides>,
    spatial_query: avian3d::prelude::SpatialQuery,
    q: Query<(
        &Transform,
        &avian3d::prelude::Collider,
        Option<&CollisionShapeData>,
    )>,
) {
    if queue.0.is_empty() {
        return;
    }
    for slide in std::mem::take(&mut queue.0) {
        let Ok((transform, collider, shape_data)) = q.get(slide.entity) else {
            continue;
        };
        // The avian Collider is offset from the entity transform via
        // ColliderTransform (see `spawn_collision_shape`). `shape_cast_slide`
        // takes the shape's world centre, so we have to add that offset
        // ourselves — otherwise the cast (especially the downward grounding
        // probe) happens at a phantom location `offset` units from the real
        // collider and the character ends up floating that distance off the
        // floor.
        let shape_offset = shape_data.map(|s| s.offset).unwrap_or(Vec3::ZERO);
        let shape_origin = transform.translation + shape_offset;
        let filter = avian3d::prelude::SpatialQueryFilter::from_excluded_entities([slide.entity]);
        let result = backend::avian_character::shape_cast_slide(
            &spatial_query,
            collider,
            shape_origin,
            transform.rotation,
            slide.delta,
            slide.max_slope,
            &filter,
        );
        let new_pos = transform.translation + result.actual_delta;
        resolved
            .0
            .push((slide.entity, new_pos, result.grounded, result.ground_normal));
    }
}

#[cfg(feature = "avian")]
fn apply_kinematic_slides(
    mut resolved: ResMut<ResolvedSlides>,
    mut q: Query<(&mut Transform, Option<&mut PhysicsReadState>)>,
) {
    if resolved.0.is_empty() {
        return;
    }
    for (entity, new_pos, grounded, normal) in std::mem::take(&mut resolved.0) {
        let Ok((mut transform, read_state)) = q.get_mut(entity) else {
            continue;
        };
        transform.translation = new_pos;
        if let Some(mut rs) = read_state {
            rs.grounded = grounded;
            rs.ground_normal = normal;
        }
    }
}

/// System to clear avian forces each frame (since we use ConstantForce for one-time pushes).
#[cfg(feature = "avian")]
fn clear_avian_forces(
    mut commands: Commands,
    query: Query<Entity, With<avian3d::prelude::ConstantForce>>,
) {
    for entity in &query {
        commands
            .entity(entity)
            .remove::<avian3d::prelude::ConstantForce>();
    }
}

/// Observer: handle physics commands (apply_force, apply_impulse, set_velocity,
/// kinematic_slide) from scripts and blueprints.
fn handle_physics_script_actions(
    trigger: On<renzora::ScriptAction>,
    mut commands: Commands,
    mut pending_slides: Option<ResMut<PendingKinematicSlides>>,
) {
    let action = trigger.event();
    let name = action.name.as_str();

    // Kinematic slide goes into a pending queue drained by a system with
    // SpatialQuery access — it can't run inside an observer.
    if name == "kinematic_slide" {
        use renzora::ScriptActionValue;
        let get = |k: &str| -> f32 {
            match action.args.get(k) {
                Some(ScriptActionValue::Float(v)) => *v,
                Some(ScriptActionValue::Int(v)) => *v as f32,
                _ => 0.0,
            }
        };
        let dx = get("x");
        let dy = get("y");
        let dz = get("z");
        let max_slope = match action.args.get("max_slope") {
            Some(ScriptActionValue::Float(v)) => *v,
            _ => 55.0,
        };
        if let Some(ref mut queue) = pending_slides {
            queue.0.push(PendingSlide {
                entity: action.entity,
                delta: Vec3::new(dx, dy, dz),
                max_slope,
            });
        }
        return;
    }

    if !matches!(name, "apply_force" | "apply_impulse" | "set_velocity") {
        return;
    }

    use renzora::ScriptActionValue;
    let x = match action.args.get("x") {
        Some(ScriptActionValue::Float(v)) => *v,
        _ => 0.0,
    };
    let y = match action.args.get("y") {
        Some(ScriptActionValue::Float(v)) => *v,
        _ => 0.0,
    };
    let z = match action.args.get("z") {
        Some(ScriptActionValue::Float(v)) => *v,
        _ => 0.0,
    };
    let vec = Vec3::new(x, y, z);

    // Default to the entity that triggered the action, or use target ID if provided
    let target =
        if let Some(Some(ScriptActionValue::Int(id))) = action.args.get("entity_id").map(Some) {
            Entity::from_bits(*id as u64)
        } else {
            action.entity
        };

    match name {
        "apply_force" => {
            #[cfg(feature = "avian")]
            commands
                .entity(target)
                .insert(avian3d::prelude::ConstantForce(vec));
            #[cfg(feature = "rapier")]
            { /* TODO: rapier apply_force */ }
        }
        "apply_impulse" => {
            #[cfg(feature = "avian")]
            {
                // Avian 0.6.1 doesn't have a built-in one-shot impulse component in prelude.
                // We'll apply it by inserting LinearVelocity which avian's solver will integrate.
                // This is a simplified impulse. For a true additive impulse we'd need a solver hook.
                commands
                    .entity(target)
                    .insert(avian3d::prelude::LinearVelocity(vec));
            }
            #[cfg(feature = "rapier")]
            { /* TODO: rapier apply_impulse */ }
        }
        "set_velocity" => {
            #[cfg(feature = "avian")]
            commands
                .entity(target)
                .insert(avian3d::prelude::LinearVelocity(vec));
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
pub fn spawn_collision_shape(
    commands: &mut Commands,
    entity: Entity,
    shape_data: &CollisionShapeData,
) {
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
        (
            Entity,
            Option<&PhysicsBodyData>,
            Option<&CollisionShapeData>,
            Option<&Name>,
        ),
        (
            Without<RuntimePhysics>,
            Or<(With<PhysicsBodyData>, With<CollisionShapeData>)>,
        ),
    >,
) {
    for (entity, body, shape, name) in &new_bodies {
        let label = name.map(|n| n.as_str()).unwrap_or("unnamed");
        info!(
            "[Physics] Initialized physics on '{}' {:?} (body={}, shape={})",
            label,
            entity,
            body.is_some(),
            shape.is_some()
        );
        renzora::console_log::console_info(
            "Physics",
            format!(
                "Initialized physics on '{}' (body={}, shape={})",
                label,
                body.is_some(),
                shape.is_some()
            ),
        );
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
    changed_bodies: Query<
        (Entity, &PhysicsBodyData),
        (With<RuntimePhysics>, Changed<PhysicsBodyData>),
    >,
    changed_shapes: Query<
        (Entity, &CollisionShapeData),
        (With<RuntimePhysics>, Changed<CollisionShapeData>),
    >,
) {
    for (entity, body_data) in &changed_bodies {
        spawn_physics_body(&mut commands, entity, body_data);
    }
    for (entity, shape_data) in &changed_shapes {
        spawn_collision_shape(&mut commands, entity, shape_data);
    }
}

/// Observer: pause physics when the editor sends `PausePhysics`.
fn on_pause_physics(_trigger: On<renzora::PausePhysics>, mut commands: Commands) {
    commands.queue(|world: &mut World| pause(world));
}

/// Observer: unpause physics when the editor sends `UnpausePhysics`.
fn on_unpause_physics(_trigger: On<renzora::UnpausePhysics>, mut commands: Commands) {
    commands.queue(|world: &mut World| unpause(world));
}

/// Unpause the physics simulation.
pub fn unpause(world: &mut World) {
    info!("[Physics] Unpausing physics simulation");
    renzora::console_log::console_info("Physics", "Physics simulation unpaused");
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
    renzora::console_log::console_info("Physics", "Physics simulation paused");
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
