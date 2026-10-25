//! The engine's physics API, with no physics engine in it.
//!
//! This crate owns everything about physics that is renzora's rather than a
//! solver's: the components scenes save, routing each entity to the 2D or 3D
//! world, resolving colliders, script actions, the read-back state scripts
//! see, world settings, collide-and-slide, and the scene queries
//! particles use. What actually simulates is a backend behind
//! `renzora::physics::PhysicsBackend`, registered by a native plugin
//! (`avian_physics`, `rapier_physics`). The engine links none.
//!
//! Without any backend this crate still builds and runs: entities are routed
//! and colliders resolved, and nothing moves (the console says so, see
//! `selection::announce_running`). Crates like `renzora_terrain` keep tagging
//! chunks with collider data either way.

pub mod auto_fit;
pub mod data;
pub mod properties;
pub mod read_state;
pub mod resolve;
pub mod scene_query;
#[cfg(feature = "scripting")]
pub mod script_extension;
pub mod selection;
pub mod slide;

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
pub use selection::switch_backend;
pub use slide::{PendingKinematicSlides, PendingSlide};

use bevy::prelude::*;
use renzora::physics::{
    PhysicsBackends, PhysicsCommand, PhysicsCommands, PhysicsDim, PhysicsSet, PhysicsSettings,
    ResolvedCollider,
};
use renzora::PlayModeState;

/// Run condition: true when NOT in editing mode (i.e. playing, scripts-only, or no PlayModeState resource).
pub fn not_editing(play_mode: Option<Res<PlayModeState>>) -> bool {
    play_mode.is_none_or(|pm| !pm.is_editing())
}

/// The engine half of physics. Simulates nothing by itself; see the crate docs.
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

        app.register_type::<PhysicsBodyData>()
            .register_type::<PhysicsBodyType>()
            .register_type::<CollisionShapeData>()
            .register_type::<CollisionShapeType>()
            .register_type::<Physics2d>()
            .register_type::<PhysicsReadState>()
            .register_type::<read_state::CollisionReadState>();

        app.init_resource::<PhysicsBackends>()
            .init_resource::<PhysicsSettings>()
            .init_resource::<PhysicsCommands>()
            .init_resource::<PhysicsPropertiesState>()
            .init_resource::<PendingKinematicSlides>();

        app.configure_sets(
            Update,
            (
                PhysicsSet::Prepare,
                PhysicsSet::Sync,
                PhysicsSet::Writeback,
                PhysicsSet::Read,
            )
                .chain(),
        );

        // Ray and sphere casts against the colliders, for the crates that may
        // not link a physics engine themselves (particles colliding with the
        // level). With no backend registered every cast is a miss, which is the
        // same answer as an empty scene.
        app.insert_resource(renzora::scene_query::SceneQueries::new(
            scene_query::BackendSceneQuery,
        ));

        app.add_systems(
            Update,
            (
                auto_fit::mark_new_collision_shapes,
                auto_fit::auto_fit_collision_shapes,
            )
                .chain()
                .before(PhysicsSet::Prepare),
        );
        // Before routing, so a world that just changed backend is re-wired to
        // the new one in the same frame.
        app.add_systems(
            Update,
            selection::follow_project_backends.before(PhysicsSet::Prepare),
        );
        app.add_systems(
            Update,
            (
                (
                    handle_removed_physics_data,
                    auto_init_physics,
                    sync_physics_data,
                    resolve::resolve_mesh_colliders,
                    resolve::resolve_model_colliders,
                ),
                read_state::auto_init_physics_read_state,
                read_state::auto_init_collision_read_state,
            )
                .in_set(PhysicsSet::Prepare),
        );
        app.add_systems(
            Update,
            (
                read_state::update_physics_read_state,
                read_state::update_collision_read_state,
                apply_property_commands,
            )
                .in_set(PhysicsSet::Read),
        );

        // Commands land in `PostUpdate`, after every script has run; the queue
        // is emptied once each backend has taken what is its own. Slides also
        // run here, so a `move_controller` call from any script this frame is
        // in the transform before it propagates.
        app.add_systems(
            PostUpdate,
            (
                clear_physics_commands.after(PhysicsSet::Commands),
                slide::run_kinematic_slides
                    .run_if(not_editing)
                    .before(bevy::transform::TransformSystems::Propagate),
            ),
        );

        // Listen for editor pause/unpause events (decoupled from renzora_editor_framework)
        app.add_observer(on_pause_physics)
            .add_observer(on_unpause_physics);

        // Listen for script actions (apply_force, apply_impulse, set_velocity, kinematic_slide)
        app.add_observer(handle_physics_script_actions);

        // Register script functions owned by the physics crate.
        #[cfg(feature = "scripting")]
        {
            let mut extensions = app.world_mut().get_resource_or_insert_with(
                renzora_scripting::extension::ScriptExtensions::default,
            );
            extensions.register(script_extension::PhysicsScriptExtension);
        }
    }
}

fn clear_physics_commands(mut commands: ResMut<PhysicsCommands>) {
    commands.0.clear();
}

/// Observer: turn physics script actions (apply_force,
/// apply_impulse, set_velocity, kinematic_slide) into commands for the backend.
fn handle_physics_script_actions(
    trigger: On<renzora::ScriptAction>,
    mut physics: ResMut<PhysicsCommands>,
    mut pending_slides: ResMut<PendingKinematicSlides>,
) {
    use renzora::ScriptActionValue;
    let action = trigger.event();
    let name = action.name.as_str();

    // Kinematic slide goes into a pending queue drained by an exclusive
    // system, since the cast needs the whole world.
    if name == "kinematic_slide" {
        let get = |k: &str| -> f32 {
            match action.args.get(k) {
                Some(ScriptActionValue::Float(v)) => *v,
                Some(ScriptActionValue::Int(v)) => *v as f32,
                _ => 0.0,
            }
        };
        let max_slope = match action.args.get("max_slope") {
            Some(ScriptActionValue::Float(v)) => *v,
            _ => 55.0,
        };
        pending_slides.0.push(PendingSlide {
            entity: action.entity,
            delta: Vec3::new(get("x"), get("y"), get("z")),
            max_slope,
        });
        return;
    }

    if !matches!(
        name,
        "apply_force"
            | "apply_impulse"
            | "set_velocity"
            | "apply_torque"
            | "apply_angular_impulse"
            | "set_angular_velocity"
    ) {
        return;
    }

    let get = |k: &str| -> f32 {
        match action.args.get(k) {
            Some(ScriptActionValue::Float(v)) => *v,
            _ => 0.0,
        }
    };
    let vec = Vec3::new(get("x"), get("y"), get("z"));

    // Default to the entity that triggered the action, or use target ID if provided
    let entity = if let Some(ScriptActionValue::Int(id)) = action.args.get("entity_id") {
        Entity::from_bits(*id as u64)
    } else {
        action.entity
    };

    physics.push(match name {
        "apply_force" => PhysicsCommand::AddForce { entity, force: vec },
        "apply_torque" => PhysicsCommand::AddTorque { entity, torque: vec },
        "apply_angular_impulse" => PhysicsCommand::AddAngularImpulse { entity, impulse: vec },
        "set_angular_velocity" => PhysicsCommand::SetAngularVelocity { entity, velocity: vec },
        // A velocity set, not `AddImpulse`. `apply_impulse` has always set the
        // velocity to its argument, and existing scripts are tuned against
        // that; code that wants momentum pushes `AddImpulse` itself.
        "apply_impulse" => PhysicsCommand::SetLinearVelocity { entity, velocity: vec },
        _ => PhysicsCommand::SetLinearVelocity { entity, velocity: vec },
    });
}

/// Take `entity` out of the simulation: the backend drops its body and
/// collider, and the entity keeps its data and `RuntimePhysics`, so it stays
/// out until its `CollisionShapeData` changes. Terrain streams far chunks out
/// this way.
pub fn despawn_physics_components(commands: &mut Commands, entity: Entity) {
    commands.queue(move |world: &mut World| {
        if let Ok(mut e) = world.get_entity_mut(entity) {
            e.remove::<(ResolvedCollider, resolve::PendingMeshCollider)>();
        }
        world
            .resource_mut::<PhysicsCommands>()
            .push(PhysicsCommand::Strip { entity });
    });
}

/// True if this entity belongs to the **2D** physics world: it is a sprite,
/// carries the explicit [`Physics2d`] marker, or any ancestor is a `Node2d` /
/// sprite (painted tiles sit under their tilemap layer, props under a 2D
/// group node). Everything else routes to the 3D world, which keeps every
/// existing 3D scene behaving exactly as before this walk existed.
fn entity_is_2d(
    entity: Entity,
    markers_2d: &Query<(), Or<(With<Physics2d>, With<Sprite>, With<renzora::Node2d>)>>,
    parents: &Query<&ChildOf>,
) -> bool {
    let mut e = entity;
    loop {
        if markers_2d.contains(e) {
            return true;
        }
        match parents.get(e) {
            Ok(child_of) => e = child_of.parent(),
            Err(_) => return false,
        }
    }
}

/// Wire up entities that have physics data but no `RuntimePhysics` marker yet.
/// Each entity is routed to the 2D or 3D world once, here; the decision is
/// remembered via the `RuntimePhysics2d` marker.
fn auto_init_physics(
    mut commands: Commands,
    new_bodies: Query<
        (
            Entity,
            Option<&PhysicsBodyData>,
            Option<&CollisionShapeData>,
            Option<&Name>,
            Has<RuntimeOnlyPhysics>,
        ),
        (
            Without<RuntimePhysics>,
            Or<(With<PhysicsBodyData>, With<CollisionShapeData>)>,
        ),
    >,
    markers_2d: Query<(), Or<(With<Physics2d>, With<Sprite>, With<renzora::Node2d>)>>,
    parents: Query<&ChildOf>,
) {
    for (entity, body, shape, name, generated) in &new_bodies {
        let is_2d = entity_is_2d(entity, &markers_2d, &parents);
        let dim = if is_2d { PhysicsDim::D2 } else { PhysicsDim::D3 };
        if let Some(s) = shape {
            resolve::insert_resolved(&mut commands, entity, s, dim);
        }
        let mut e = commands.entity(entity);
        e.try_insert(RuntimePhysics);
        if is_2d {
            e.try_insert(RuntimePhysics2d);
        }
        // Generated bodies (every bone of a ragdoll) are announced by whatever
        // generated them, once, rather than one console line per bone.
        if generated {
            continue;
        }
        let label = name.map(|n| n.as_str()).unwrap_or("unnamed");
        info!(
            "[Physics] Initialized {} physics on '{}' {:?} (body={}, shape={})",
            if is_2d { "2D" } else { "3D" },
            label,
            entity,
            body.is_some(),
            shape.is_some()
        );
        renzora::console_log::console_info(
            "Physics",
            format!(
                "Initialized {} physics on '{}' (body={}, shape={})",
                if is_2d { "2D" } else { "3D" },
                label,
                body.is_some(),
                shape.is_some()
            ),
        );
    }
}

/// Take physics away from an entity whose data was removed while it lives.
///
/// A collider that loses its `CollisionShapeData` loses its resolved shape,
/// which a backend reads as "this collider is gone". An entity left with no
/// physics data at all loses its wiring too, which a backend reads as "all of
/// it is gone". A body that loses only its `PhysicsBodyData` is left to the
/// backend, which drops the body and keeps the collider (see
/// `renzora::physics::PhysicsSet::Sync`). Without this an undone "make solid"
/// or a component removed in the inspector left the old body simulating.
fn handle_removed_physics_data(
    mut commands: Commands,
    mut removed_shapes: RemovedComponents<CollisionShapeData>,
    mut removed_bodies: RemovedComponents<PhysicsBodyData>,
    alive: Query<(Has<PhysicsBodyData>, Has<CollisionShapeData>), With<RuntimePhysics>>,
) {
    for entity in removed_shapes.read().chain(removed_bodies.read()) {
        let Ok((has_body, has_shape)) = alive.get(entity) else { continue };
        let mut e = commands.entity(entity);
        if !has_shape {
            e.try_remove::<(ResolvedCollider, resolve::PendingMeshCollider)>();
        }
        if !has_body && !has_shape {
            e.try_remove::<(RuntimePhysics, RuntimePhysics2d)>();
        }
    }
}

/// Re-resolve a collider when its `CollisionShapeData` changes at runtime. A
/// changed `PhysicsBodyData` needs nothing from the engine: the backend
/// watches it directly.
fn sync_physics_data(
    mut commands: Commands,
    changed_shapes: Query<
        (Entity, &CollisionShapeData, Has<RuntimePhysics2d>),
        (With<RuntimePhysics>, Changed<CollisionShapeData>),
    >,
) {
    for (entity, shape_data, is_2d) in &changed_shapes {
        let dim = if is_2d { PhysicsDim::D2 } else { PhysicsDim::D3 };
        resolve::insert_resolved(&mut commands, entity, shape_data, dim);
    }
}

/// Apply the Physics panel's queued edits to `PhysicsSettings`, and mirror the
/// result back so the panel shows what is in effect.
fn apply_property_commands(
    mut state: ResMut<PhysicsPropertiesState>,
    mut settings: ResMut<PhysicsSettings>,
) {
    if state.commands.is_empty() {
        if state.gravity != settings.gravity || state.substeps != settings.substeps {
            state.gravity = settings.gravity;
            state.substeps = settings.substeps;
        }
        return;
    }
    let cmds: Vec<PhysicsPropertyCommand> = state.commands.drain(..).collect();
    for cmd in cmds {
        match cmd {
            PhysicsPropertyCommand::SetGravity(g) => {
                settings.gravity = g;
                state.gravity = g;
                state.gravity_preset = GravityPreset::Custom;
            }
            PhysicsPropertyCommand::SetGravityPreset(preset) => {
                let g = preset.gravity_vec();
                settings.gravity = g;
                state.gravity = g;
                state.gravity_preset = preset;
            }
            PhysicsPropertyCommand::SetTimeScale(scale) => {
                settings.time_scale = scale;
                state.time_scale = scale;
            }
            PhysicsPropertyCommand::SetSubsteps(count) => {
                settings.substeps = count;
                state.substeps = count;
            }
            PhysicsPropertyCommand::ResetAll => {
                let defaults = PhysicsSettings::default();
                settings.gravity = defaults.gravity;
                settings.time_scale = defaults.time_scale;
                settings.substeps = defaults.substeps;
                state.gravity = defaults.gravity;
                state.gravity_preset = GravityPreset::Earth;
                state.time_scale = defaults.time_scale;
                state.substeps = defaults.substeps;
            }
        }
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

/// Unpause the physics simulation (both the 2D and 3D worlds: each backend
/// keeps its own clock, but pause/play is one editor concept).
pub fn unpause(world: &mut World) {
    info!("[Physics] Unpausing physics simulation");
    renzora::console_log::console_info("Physics", "Physics simulation unpaused");
    world.get_resource_or_insert_with(PhysicsSettings::default).paused = false;
}

/// Pause the physics simulation (both the 2D and 3D worlds).
pub fn pause(world: &mut World) {
    info!("[Physics] Pausing physics simulation");
    renzora::console_log::console_info("Physics", "Physics simulation paused");
    world.get_resource_or_insert_with(PhysicsSettings::default).paused = true;
}
