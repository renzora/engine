//! Command processing system — applies transform writes and routes ScriptCommands.

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};

use super::execution::{ScriptCommandQueue, ScriptEnvironmentCommands, ScriptLogBuffer, ScriptLogEntry, ScriptReflectionQueue, ReflectionSet};
use crate::command::{ScriptCommand, CharacterCommand, CharacterCommandQueue};
use crate::resources::ScriptTimers;

/// System that applies script outputs to the world.
///
/// Runs in `ScriptingSet::CommandProcessing`.
pub fn apply_script_commands(
    mut transforms: Query<&mut Transform>,
    mut cmd_queue: ResMut<ScriptCommandQueue>,
    mut commands: Commands,
    mut timers: ResMut<ScriptTimers>,
    mut visibility_query: Query<&mut Visibility>,
    _name_query: Query<(Entity, &Name)>,
    mut log_buffer: ResMut<ScriptLogBuffer>,
    mut pending_env: ResMut<ScriptEnvironmentCommands>,
    mut reflection_queue: ResMut<ScriptReflectionQueue>,
    mut pending_scene: ResMut<renzora::PendingSceneLoad>,
    mut character_queue: ResMut<CharacterCommandQueue>,
    mut cursor_query: Query<&mut CursorOptions>,
    mut tw_queue: ResMut<renzora::TransformWriteQueue>,
    mut ran_once: Local<bool>,
) {
    if !*ran_once {
        renzora::clog_info!("ScriptCmd", "apply_script_commands is RUNNING (first time)");
        *ran_once = true;
    }

    // 0. Drain TransformWriteQueue from core (blueprint writes go here)
    if !tw_queue.writes.is_empty() {
        cmd_queue.transform_writes.extend(tw_queue.writes.drain(..));
    }

    // 1. Apply transform writes
    let tw_count = cmd_queue.transform_writes.len();
    if tw_count > 0 {
        renzora::clog_info!("ScriptCmd", "apply_script_commands: {} transform_writes", tw_count);
    }
    for tw in cmd_queue.transform_writes.drain(..) {
        renzora::clog_info!("ScriptCmd", "TW entity={:?} rot_delta={:?}", tw.entity, tw.rotation_delta);
        let Ok(mut t) = transforms.get_mut(tw.entity) else {
            renzora::clog_warn!("ScriptCmd", "Entity {:?} NOT FOUND in query!", tw.entity);
            continue;
        };

        if let Some(pos) = tw.new_position {
            t.translation = pos;
        }
        if let Some(rot) = tw.new_rotation {
            t.rotation = Quat::from_euler(
                EulerRot::YXZ,
                rot.y.to_radians(),
                rot.x.to_radians(),
                rot.z.to_radians(),
            );
        }
        if let Some(trans) = tw.translation {
            t.translation += trans;
        }
        if let Some(delta) = tw.rotation_delta {
            t.rotation *= Quat::from_euler(
                EulerRot::YXZ,
                delta.y.to_radians(),
                delta.x.to_radians(),
                delta.z.to_radians(),
            );
        }
        if let Some(scale) = tw.new_scale {
            t.scale = scale;
        }
        if let Some(target) = tw.look_at {
            t.look_at(target, Vec3::Y);
        }
    }

    // 2. Process general commands
    for (source_entity, cmd) in cmd_queue.commands.drain(..) {
        match cmd {
            // === ECS ===
            ScriptCommand::SpawnEntity { name } => {
                commands.spawn((
                    Name::new(name),
                    Transform::default(),
                    Visibility::default(),
                ));
            }
            ScriptCommand::SpawnPrimitive { name, primitive_type: _, position, scale } => {
                let mut t = Transform::default();
                if let Some(pos) = position {
                    t.translation = pos;
                }
                if let Some(s) = scale {
                    t.scale = s;
                }
                commands.spawn((
                    Name::new(name),
                    t,
                    Visibility::default(),
                ));
            }
            ScriptCommand::DespawnEntity { entity_id } => {
                let e = Entity::from_bits(entity_id);
                commands.entity(e).despawn();
            }
            ScriptCommand::DespawnSelf => {
                commands.entity(source_entity).despawn();
            }

            // === Visibility ===
            ScriptCommand::SetVisibility { entity_id, visible } => {
                let target = entity_id.map(|id| Entity::from_bits(id)).unwrap_or(source_entity);
                if let Ok(mut vis) = visibility_query.get_mut(target) {
                    *vis = if visible { Visibility::Inherited } else { Visibility::Hidden };
                }
            }

            // === Timers ===
            ScriptCommand::StartTimer { name, duration, repeat: repeating } => {
                timers.start(&name, duration, repeating);
            }
            ScriptCommand::StopTimer { name } => {
                timers.stop(&name);
            }
            ScriptCommand::PauseTimer { name } => {
                timers.pause(&name);
            }
            ScriptCommand::ResumeTimer { name } => {
                timers.resume(&name);
            }

            // === Debug ===
            ScriptCommand::Log { level, message } => {
                match level.as_str() {
                    "warn" => renzora::clog_warn!("Script", "{}", message),
                    "error" => renzora::clog_error!("Script", "{}", message),
                    _ => renzora::clog_info!("Script", "{}", message),
                }
                log_buffer.entries.push(ScriptLogEntry {
                    level: level.clone(),
                    message: message.clone(),
                });
            }

            // === Entity name ===
            ScriptCommand::SetEntityName { entity_id, name } => {
                let target = Entity::from_bits(entity_id);
                commands.entity(target).try_insert(Name::new(name));
            }

            // === Environment ===
            ScriptCommand::SetSunAngles { azimuth, elevation } => {
                // Route via ScriptAction so renzora_lighting can observe it
                let entity = source_entity;
                let mut args = std::collections::HashMap::new();
                args.insert("azimuth".to_string(), renzora::ScriptActionValue::Float(azimuth));
                args.insert("elevation".to_string(), renzora::ScriptActionValue::Float(elevation));
                commands.queue(move |world: &mut World| {
                    world.trigger(renzora::ScriptAction {
                        name: "set_sun_angles".to_string(),
                        entity,
                        target_entity: None,
                        args,
                    });
                });
            }

            // === Generic Reflection ===
            ScriptCommand::SetComponentField { entity_id, entity_name, component_type, field_path, value } => {
                reflection_queue.sets.push(ReflectionSet {
                    source_entity,
                    entity_id,
                    entity_name,
                    component_type,
                    field_path,
                    value,
                });
            }

            // === Scene ===
            ScriptCommand::LoadScene { path } => {
                renzora::clog_info!("Scene", "LoadScene requested: {}", path);
                pending_scene.requests.push(path);
            }

            // === Character Controller ===
            ScriptCommand::CharacterMove { direction } => {
                character_queue.commands.push((source_entity, CharacterCommand::Move(direction)));
            }
            ScriptCommand::CharacterJump => {
                character_queue.commands.push((source_entity, CharacterCommand::Jump));
            }
            ScriptCommand::CharacterSprint { sprinting } => {
                character_queue.commands.push((source_entity, CharacterCommand::Sprint(sprinting)));
            }

            // === Cursor ===
            ScriptCommand::LockCursor => {
                if let Ok(mut cursor) = cursor_query.single_mut() {
                    cursor.grab_mode = CursorGrabMode::Locked;
                    cursor.visible = false;
                }
            }
            ScriptCommand::UnlockCursor => {
                if let Ok(mut cursor) = cursor_query.single_mut() {
                    cursor.grab_mode = CursorGrabMode::None;
                    cursor.visible = true;
                }
            }

            // Generic script action — triggers a ScriptAction event that
            // domain crates observe (decoupled, no import needed).
            ScriptCommand::Action { name, target_entity, args } => {
                let entity = source_entity;
                commands.queue(move |world: &mut World| {
                    world.trigger(renzora::ScriptAction {
                        name,
                        entity,
                        target_entity,
                        args,
                    });
                });
            }

            // All remaining commands are routed as ScriptAction events.
            // Domain crates (physics, animation, audio, etc.) observe these.
            other => {
                let entity = source_entity;
                let debug_msg = format!("{:?}", other);
                if let Some((name, args)) = script_command_to_action(other) {
                    commands.queue(move |world: &mut World| {
                        world.trigger(renzora::ScriptAction {
                            name,
                            entity,
                            target_entity: None,
                            args,
                        });
                    });
                } else {
                    debug!("Unhandled script command: {}", debug_msg);
                }
            }
        }
    }
}

/// Convert a ScriptCommand to a (name, args) pair for ScriptAction routing.
/// Returns None for commands that can't be meaningfully converted.
fn script_command_to_action(
    cmd: ScriptCommand,
) -> Option<(String, std::collections::HashMap<String, renzora::ScriptActionValue>)> {
    use renzora::ScriptActionValue as V;
    let mut args = std::collections::HashMap::new();

    let name = match cmd {
        // Physics
        ScriptCommand::ApplyForce { entity_id, force } => {
            if let Some(id) = entity_id { args.insert("entity_id".into(), V::Int(id as i64)); }
            args.insert("x".into(), V::Float(force.x));
            args.insert("y".into(), V::Float(force.y));
            args.insert("z".into(), V::Float(force.z));
            "apply_force"
        }
        ScriptCommand::ApplyImpulse { entity_id, impulse } => {
            if let Some(id) = entity_id { args.insert("entity_id".into(), V::Int(id as i64)); }
            args.insert("x".into(), V::Float(impulse.x));
            args.insert("y".into(), V::Float(impulse.y));
            args.insert("z".into(), V::Float(impulse.z));
            "apply_impulse"
        }
        ScriptCommand::SetVelocity { entity_id, velocity } => {
            if let Some(id) = entity_id { args.insert("entity_id".into(), V::Int(id as i64)); }
            args.insert("x".into(), V::Float(velocity.x));
            args.insert("y".into(), V::Float(velocity.y));
            args.insert("z".into(), V::Float(velocity.z));
            "set_velocity"
        }
        ScriptCommand::SetGravityScale { entity_id, scale } => {
            if let Some(id) = entity_id { args.insert("entity_id".into(), V::Int(id as i64)); }
            args.insert("scale".into(), V::Float(scale));
            "set_gravity_scale"
        }

        // Animation
        ScriptCommand::PlayAnimation { entity_id, name, looping, speed } => {
            if let Some(id) = entity_id { args.insert("entity_id".into(), V::Int(id as i64)); }
            args.insert("name".into(), V::String(name));
            args.insert("looping".into(), V::Bool(looping));
            args.insert("speed".into(), V::Float(speed));
            "play_animation"
        }
        ScriptCommand::StopAnimation { entity_id } => {
            if let Some(id) = entity_id { args.insert("entity_id".into(), V::Int(id as i64)); }
            "stop_animation"
        }
        ScriptCommand::PauseAnimation { entity_id } => {
            if let Some(id) = entity_id { args.insert("entity_id".into(), V::Int(id as i64)); }
            "pause_animation"
        }
        ScriptCommand::ResumeAnimation { entity_id } => {
            if let Some(id) = entity_id { args.insert("entity_id".into(), V::Int(id as i64)); }
            "resume_animation"
        }
        ScriptCommand::SetAnimationSpeed { entity_id, speed } => {
            if let Some(id) = entity_id { args.insert("entity_id".into(), V::Int(id as i64)); }
            args.insert("speed".into(), V::Float(speed));
            "set_animation_speed"
        }
        ScriptCommand::CrossfadeAnimation { entity_id, name, duration, looping } => {
            if let Some(id) = entity_id { args.insert("entity_id".into(), V::Int(id as i64)); }
            args.insert("name".into(), V::String(name));
            args.insert("duration".into(), V::Float(duration));
            args.insert("looping".into(), V::Bool(looping));
            "crossfade_animation"
        }
        ScriptCommand::SetAnimationParam { entity_id, name, value } => {
            if let Some(id) = entity_id { args.insert("entity_id".into(), V::Int(id as i64)); }
            args.insert("name".into(), V::String(name));
            args.insert("value".into(), V::Float(value));
            "set_anim_param"
        }
        ScriptCommand::SetAnimationBoolParam { entity_id, name, value } => {
            if let Some(id) = entity_id { args.insert("entity_id".into(), V::Int(id as i64)); }
            args.insert("name".into(), V::String(name));
            args.insert("value".into(), V::Bool(value));
            "set_anim_bool"
        }
        ScriptCommand::TriggerAnimation { entity_id, name } => {
            if let Some(id) = entity_id { args.insert("entity_id".into(), V::Int(id as i64)); }
            args.insert("name".into(), V::String(name));
            "trigger_anim"
        }
        ScriptCommand::SetAnimationLayerWeight { entity_id, layer_name, weight } => {
            if let Some(id) = entity_id { args.insert("entity_id".into(), V::Int(id as i64)); }
            args.insert("layer_name".into(), V::String(layer_name));
            args.insert("weight".into(), V::Float(weight));
            "set_layer_weight"
        }

        // Tweens
        ScriptCommand::TweenPosition { entity_id, target, duration, easing } => {
            if let Some(id) = entity_id { args.insert("entity_id".into(), V::Int(id as i64)); }
            args.insert("target".into(), V::Vec3([target.x, target.y, target.z]));
            args.insert("duration".into(), V::Float(duration));
            args.insert("easing".into(), V::String(easing));
            "tween_position"
        }
        ScriptCommand::TweenRotation { entity_id, target, duration, easing } => {
            if let Some(id) = entity_id { args.insert("entity_id".into(), V::Int(id as i64)); }
            args.insert("target".into(), V::Vec3([target.x, target.y, target.z]));
            args.insert("duration".into(), V::Float(duration));
            args.insert("easing".into(), V::String(easing));
            "tween_rotation"
        }
        ScriptCommand::TweenScale { entity_id, target, duration, easing } => {
            if let Some(id) = entity_id { args.insert("entity_id".into(), V::Int(id as i64)); }
            args.insert("target".into(), V::Vec3([target.x, target.y, target.z]));
            args.insert("duration".into(), V::Float(duration));
            args.insert("easing".into(), V::String(easing));
            "tween_scale"
        }

        // Audio
        ScriptCommand::PlaySound { path, volume, looping, bus } => {
            args.insert("path".into(), V::String(path));
            args.insert("volume".into(), V::Float(volume));
            args.insert("looping".into(), V::Bool(looping));
            args.insert("bus".into(), V::String(bus));
            "play_sound"
        }
        ScriptCommand::PlayMusic { path, volume, fade_in, bus } => {
            args.insert("path".into(), V::String(path));
            args.insert("volume".into(), V::Float(volume));
            args.insert("fade_in".into(), V::Float(fade_in));
            args.insert("bus".into(), V::String(bus));
            "play_music"
        }
        ScriptCommand::StopMusic { fade_out } => {
            args.insert("fade_out".into(), V::Float(fade_out));
            "stop_music"
        }
        ScriptCommand::StopAllSounds => "stop_all_sounds",

        // Rendering
        ScriptCommand::SetMaterialColor { entity_id, color } => {
            if let Some(id) = entity_id { args.insert("entity_id".into(), V::Int(id as i64)); }
            args.insert("r".into(), V::Float(color[0]));
            args.insert("g".into(), V::Float(color[1]));
            args.insert("b".into(), V::Float(color[2]));
            args.insert("a".into(), V::Float(color[3]));
            "set_material_color"
        }
        ScriptCommand::SetLightIntensity { entity_id, intensity } => {
            if let Some(id) = entity_id { args.insert("entity_id".into(), V::Int(id as i64)); }
            args.insert("intensity".into(), V::Float(intensity));
            "set_light_intensity"
        }
        ScriptCommand::SetLightColor { entity_id, color } => {
            if let Some(id) = entity_id { args.insert("entity_id".into(), V::Int(id as i64)); }
            args.insert("r".into(), V::Float(color[0]));
            args.insert("g".into(), V::Float(color[1]));
            args.insert("b".into(), V::Float(color[2]));
            "set_light_color"
        }

        // Camera
        ScriptCommand::ScreenShake { intensity, duration } => {
            args.insert("intensity".into(), V::Float(intensity));
            args.insert("duration".into(), V::Float(duration));
            "screen_shake"
        }

        _ => return None,
    };

    Some((name.to_string(), args))
}
