//! Core script execution system — queries entities with ScriptComponent,
//! builds a ScriptContext for each script entry, and calls on_ready/on_update.

use bevy::prelude::*;
use std::collections::HashMap;

use renzora::EntityTag;

use crate::command::ScriptCommand;
use crate::component::ScriptComponent;
use crate::context::{ChildNodeInfo, ScriptContext, ScriptTime, ScriptTransform};
use crate::engine::ScriptEngine;
use crate::input::ScriptInput;
use crate::resources::ScriptTimers;

/// Collected commands from all script executions this frame.
#[derive(Resource, Default)]
pub struct ScriptCommandQueue {
    /// (source_entity, command) pairs to process.
    pub commands: Vec<(Entity, ScriptCommand)>,
    /// Transform outputs applied directly by the execution system.
    pub transform_writes: Vec<TransformWrite>,
}

/// Pending environment commands for external systems to consume.
#[derive(Resource, Default)]
pub struct ScriptEnvironmentCommands {
    pub sun_angles: Option<(f32, f32)>,
}

/// Pending reflection-based component field writes.
#[derive(Resource, Default)]
pub struct ScriptReflectionQueue {
    pub sets: Vec<ReflectionSet>,
}

/// A single deferred reflection field write.
pub struct ReflectionSet {
    pub source_entity: Entity,
    pub entity_id: Option<u64>,
    pub entity_name: Option<String>,
    pub component_type: String,
    pub field_path: String,
    pub value: crate::command::PropertyValue,
}

/// Buffer of script log messages for external consumers (e.g. editor console).
#[derive(Resource, Default)]
pub struct ScriptLogBuffer {
    pub entries: Vec<ScriptLogEntry>,
}

/// A single script log entry.
pub struct ScriptLogEntry {
    pub level: String,
    pub message: String,
}

// Re-export from renzora
pub use renzora::TransformWrite;

/// Exclusive system that executes scripts on all entities with a ScriptComponent.
///
/// Uses exclusive world access so scripts can read component fields via `get()`.
pub fn run_scripts(world: &mut World) {
    // Extract resources we need (take ownership to avoid borrow conflicts)
    let time_elapsed = world.resource::<Time>().elapsed_secs_f64();
    let time_delta = world.resource::<Time>().delta_secs();

    let input = world.resource::<ScriptInput>().clone();
    let timers_finished = world.resource::<ScriptTimers>().get_just_finished();

    // Snapshot the InputMap's action state so scripts can read it by name.
    // `renzora::ActionState` is populated each frame by renzora_input's
    // `update_action_state` system. If the resource isn't present (running
    // without the InputPlugin) we just expose empty maps.
    let mut action_pressed: HashMap<String, bool> = HashMap::new();
    let mut action_just_pressed: HashMap<String, bool> = HashMap::new();
    let mut action_just_released: HashMap<String, bool> = HashMap::new();
    let mut action_axis_1d: HashMap<String, f32> = HashMap::new();
    let mut action_axis_2d: HashMap<String, Vec2> = HashMap::new();
    if let Some(state) = world.get_resource::<renzora::ActionState>() {
        for (name, data) in &state.actions {
            action_pressed.insert(name.clone(), data.pressed);
            action_just_pressed.insert(name.clone(), data.just_pressed);
            action_just_released.insert(name.clone(), data.just_released);
            action_axis_1d.insert(name.clone(), data.axis_1d);
            action_axis_2d.insert(name.clone(), data.axis_2d);
        }
    }

    // Note: do NOT clear the command queue here — other systems (e.g. blueprints)
    // may have already pushed writes this frame. The queue is drained by
    // apply_script_commands in the CommandProcessing set.

    // Build entity lookup tables (by Name, then by EntityTag — tags take priority)
    let mut entities_by_name: HashMap<String, u64> = HashMap::new();
    let mut name_to_entity: HashMap<String, Entity> = HashMap::new();
    {
        let mut query = world.query::<(Entity, &Name)>();
        for (e, n) in query.iter(world) {
            let name = n.as_str().to_string();
            entities_by_name.insert(name.clone(), e.to_bits());
            name_to_entity.insert(name, e);
        }
    }
    {
        let mut query = world.query::<(Entity, &EntityTag)>();
        for (e, tag) in query.iter(world) {
            if !tag.tag.is_empty() {
                entities_by_name.insert(tag.tag.clone(), e.to_bits());
                name_to_entity.insert(tag.tag.clone(), e);
            }
        }
    }

    let script_time = ScriptTime {
        elapsed: time_elapsed,
        delta: time_delta,
        fixed_delta: 1.0 / 60.0,
        frame_count: 0,
    };

    // Collect input into context-friendly format
    let mut keys_pressed = HashMap::new();
    let mut keys_just_pressed = HashMap::new();
    let mut keys_just_released = HashMap::new();
    for (key, &pressed) in &input.keys_pressed {
        if pressed {
            keys_pressed.insert(format!("{:?}", key), true);
        }
    }
    for (key, &pressed) in &input.keys_just_pressed {
        if pressed {
            keys_just_pressed.insert(format!("{:?}", key), true);
        }
    }
    for (key, &released) in &input.keys_just_released {
        if released {
            keys_just_released.insert(format!("{:?}", key), true);
        }
    }

    let mouse_buttons_pressed = [
        input.mouse_pressed.get(&MouseButton::Left).copied().unwrap_or(false),
        input.mouse_pressed.get(&MouseButton::Right).copied().unwrap_or(false),
        input.mouse_pressed.get(&MouseButton::Middle).copied().unwrap_or(false),
        false,
        false,
    ];
    let mouse_buttons_just_pressed = [
        input.mouse_just_pressed.get(&MouseButton::Left).copied().unwrap_or(false),
        input.mouse_just_pressed.get(&MouseButton::Right).copied().unwrap_or(false),
        input.mouse_just_pressed.get(&MouseButton::Middle).copied().unwrap_or(false),
        false,
        false,
    ];

    let gamepad_left = input.get_gamepad_left_stick(0);
    let gamepad_right = input.get_gamepad_right_stick(0);

    use bevy::input::gamepad::GamepadButton;
    let gamepad_button_list = [
        GamepadButton::South, GamepadButton::East, GamepadButton::West, GamepadButton::North,
        GamepadButton::LeftTrigger, GamepadButton::RightTrigger,
        GamepadButton::LeftTrigger2, GamepadButton::RightTrigger2,
        GamepadButton::Select, GamepadButton::Start,
        GamepadButton::LeftThumb, GamepadButton::RightThumb,
        GamepadButton::DPadUp, GamepadButton::DPadDown,
        GamepadButton::DPadLeft, GamepadButton::DPadRight,
    ];
    let mut gamepad_buttons = [false; 16];
    for (i, btn) in gamepad_button_list.iter().enumerate() {
        gamepad_buttons[i] = input.is_gamepad_button_pressed(0, *btn);
    }

    // Collect all script entities and their data
    struct ScriptEntityData {
        entity: Entity,
        entity_name: String,
        transform: Transform,
        parent: Option<Entity>,
        children: Vec<Entity>,
    }

    let mut script_entities: Vec<ScriptEntityData> = Vec::new();
    {
        let mut query = world.query::<(Entity, &ScriptComponent, &Transform, Option<&Name>, Option<&ChildOf>, Option<&Children>)>();
        for (entity, _sc, transform, name, parent, children) in query.iter(world) {
            script_entities.push(ScriptEntityData {
                entity,
                entity_name: name
                    .map(|n| n.as_str().to_string())
                    .unwrap_or_else(|| format!("Entity_{}", entity.index())),
                transform: *transform,
                parent: parent.map(|p| p.0),
                children: children.map(|c| c.iter().collect()).unwrap_or_default(),
            });
        }
    }

    // Process each script entity
    for sed in &script_entities {
        // Get parent/child transforms
        let parent_transform = sed.parent.and_then(|p| world.get::<Transform>(p).copied());
        let child_infos: Vec<(Entity, String, Transform)> = sed.children.iter().filter_map(|&child_e| {
            let t = world.get::<Transform>(child_e)?;
            let name = world.get::<Name>(child_e)
                .map(|n| n.as_str().to_string())
                .unwrap_or_else(|| format!("Entity_{}", child_e.index()));
            Some((child_e, name, *t))
        }).collect();

        // Take the ScriptComponent off the entity so we can use world freely
        let Some(mut sc) = world.entity_mut(sed.entity).take::<ScriptComponent>() else { continue };

        for entry in sc.scripts.iter_mut() {
            if !entry.enabled { continue; }
            let script_path = match &entry.script_path {
                Some(p) => p.clone(),
                None => continue,
            };

            // Build context
            let mut ctx = ScriptContext::new(
                script_time,
                ScriptTransform::from_transform(&sed.transform),
            );

            ctx.self_entity = Some(sed.entity);
            ctx.self_entity_id = sed.entity.to_bits();
            ctx.self_entity_name = sed.entity_name.clone();
            ctx.found_entities = entities_by_name.clone();

            // Input
            ctx.input_movement = input.get_movement_vector();
            ctx.mouse_position = input.mouse_position;
            ctx.mouse_delta = input.mouse_delta;
            ctx.mouse_scroll = input.scroll_delta.y;
            ctx.keys_pressed = keys_pressed.clone();
            ctx.keys_just_pressed = keys_just_pressed.clone();
            ctx.keys_just_released = keys_just_released.clone();
            ctx.mouse_buttons_pressed = mouse_buttons_pressed;
            ctx.mouse_buttons_just_pressed = mouse_buttons_just_pressed;

            // Actions (InputMap-based, unified keyboard + gamepad)
            ctx.action_pressed = action_pressed.clone();
            ctx.action_just_pressed = action_just_pressed.clone();
            ctx.action_just_released = action_just_released.clone();
            ctx.action_axis_1d = action_axis_1d.clone();
            ctx.action_axis_2d = action_axis_2d.clone();

            // Gamepad
            ctx.gamepad_left_stick = gamepad_left;
            ctx.gamepad_right_stick = gamepad_right;
            ctx.gamepad_left_trigger = input.get_gamepad_trigger(0, true);
            ctx.gamepad_right_trigger = input.get_gamepad_trigger(0, false);
            ctx.gamepad_buttons = gamepad_buttons;

            // Timers
            ctx.timers_just_finished = timers_finished.clone();

            // Parent
            if let (Some(parent_e), Some(parent_t)) = (sed.parent, &parent_transform) {
                ctx.has_parent = true;
                ctx.parent_entity = Some(parent_e);
                ctx.parent_position = parent_t.translation;
                let (y, x, z) = parent_t.rotation.to_euler(EulerRot::YXZ);
                ctx.parent_rotation = Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees());
                ctx.parent_scale = parent_t.scale;
            }

            // Children
            for (child_e, child_name, child_t) in &child_infos {
                let (ry, rx, rz) = child_t.rotation.to_euler(EulerRot::YXZ);
                ctx.children.push(ChildNodeInfo {
                    entity: *child_e,
                    name: child_name.clone(),
                    position: child_t.translation,
                    rotation: Vec3::new(rx.to_degrees(), ry.to_degrees(), rz.to_degrees()),
                    scale: child_t.scale,
                });
            }

            // Script extensions: populate custom data and set pointer for backends
            if let Some(extensions) = world.get_resource::<crate::extension::ScriptExtensions>() {
                extensions.populate_context(world, sed.entity, &mut ctx.extension_data);
                ctx.extensions_ptr = Some(extensions as *const crate::extension::ScriptExtensions);
            }

            // Set up the get handler so scripts can read component fields
            let self_entity = sed.entity;
            let name_map = name_to_entity.clone();
            let world_ptr = world as *const World;
            crate::get_handler::set_get_handler(Box::new({
                let name_map = name_map.clone();
                move |entity_name, component_type, field_path| {
                    let world_ref = unsafe { &*world_ptr };
                    let target = if let Some(name) = entity_name {
                        *name_map.get(name)?
                    } else {
                        self_entity
                    };
                    super::reflection::get_reflected_field(world_ref, target, component_type, field_path)
                }
            }));

            // Set up get_component handler (returns all fields as a map)
            crate::get_handler::set_get_component_handler(Box::new({
                let name_map = name_map.clone();
                move |entity_name, component_type| {
                    let world_ref = unsafe { &*world_ptr };
                    let target = if let Some(name) = entity_name {
                        *name_map.get(name)?
                    } else {
                        Some(self_entity)?
                    };
                    super::reflection::get_all_component_fields(world_ref, target, component_type)
                }
            }));

            // Set up get_components handler (lists component names)
            crate::get_handler::set_get_components_handler(Box::new({
                let name_map = name_map.clone();
                move |entity_name| {
                    let world_ref = unsafe { &*world_ptr };
                    let target = if let Some(name) = entity_name {
                        match name_map.get(name) {
                            Some(&e) => e,
                            None => return Vec::new(),
                        }
                    } else {
                        self_entity
                    };
                    super::reflection::get_entity_component_names(world_ref, target)
                }
            }));

            // Execute script
            let engine = world.resource::<ScriptEngine>();
            if !entry.runtime_state.initialized {
                if entry.variables.iter_all().next().is_none() {
                    let props = engine.get_script_props(&script_path);
                    for prop in &props {
                        entry.variables.set(prop.name.clone(), prop.default_value.clone());
                    }
                }

                if let Err(e) = engine.call_on_ready(&script_path, &mut ctx, &mut entry.variables) {
                    warn!("Script on_ready error [{}]: {}", script_path.display(), e);
                    entry.runtime_state.has_error = true;
                }
                entry.runtime_state.initialized = true;
            }

            if let Err(e) = engine.call_on_update(&script_path, &mut ctx, &mut entry.variables) {
                if !entry.runtime_state.has_error {
                    warn!("Script on_update error [{}]: {}", script_path.display(), e);
                    entry.runtime_state.has_error = true;
                }
            } else {
                entry.runtime_state.has_error = false;
            }

            // Clear the get handler before any mutable world access
            crate::get_handler::clear_get_handler();

            // Collect transform outputs
            let mut cmd_queue = world.resource_mut::<ScriptCommandQueue>();

            if ctx.new_position.is_some()
                || ctx.new_rotation.is_some()
                || ctx.translation.is_some()
                || ctx.rotation_delta.is_some()
                || ctx.new_scale.is_some()
                || ctx.look_at_target.is_some()
            {
                cmd_queue.transform_writes.push(TransformWrite {
                    entity: sed.entity,
                    new_position: ctx.new_position,
                    new_rotation: ctx.new_rotation,
                    translation: ctx.translation,
                    rotation_delta: ctx.rotation_delta,
                    new_scale: ctx.new_scale,
                    look_at: ctx.look_at_target,
                });
            }

            // Parent transform outputs
            if let Some(parent_e) = sed.parent {
                if ctx.parent_new_position.is_some()
                    || ctx.parent_new_rotation.is_some()
                    || ctx.parent_translation.is_some()
                {
                    cmd_queue.transform_writes.push(TransformWrite {
                        entity: parent_e,
                        new_position: ctx.parent_new_position,
                        new_rotation: ctx.parent_new_rotation,
                        translation: ctx.parent_translation,
                        rotation_delta: None,
                        new_scale: None,
                        look_at: None,
                    });
                }
            }

            // Child transform outputs
            for (child_name, change) in &ctx.child_changes {
                for (child_e, cn, _) in &child_infos {
                    if cn == child_name {
                        cmd_queue.transform_writes.push(TransformWrite {
                            entity: *child_e,
                            new_position: change.new_position,
                            new_rotation: change.new_rotation,
                            translation: change.translation,
                            rotation_delta: None,
                            new_scale: None,
                            look_at: None,
                        });
                        break;
                    }
                }
            }

            // Collect general commands
            for cmd in ctx.commands.drain(..) {
                cmd_queue.commands.push((sed.entity, cmd));
            }
        }

        // Put the ScriptComponent back on the entity
        world.entity_mut(sed.entity).insert(sc);
    }
}

