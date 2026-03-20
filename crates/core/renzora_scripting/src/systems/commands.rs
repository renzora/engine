//! Command processing system — applies transform writes and routes ScriptCommands.

use bevy::prelude::*;

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
    mut pending_scene: ResMut<renzora_core::PendingSceneLoad>,
    mut character_queue: ResMut<CharacterCommandQueue>,
    mut ran_once: Local<bool>,
) {
    if !*ran_once {
        renzora_core::clog_info!("ScriptCmd", "apply_script_commands is RUNNING (first time)");
        *ran_once = true;
    }
    // 1. Apply transform writes
    let tw_count = cmd_queue.transform_writes.len();
    if tw_count > 0 {
        renzora_core::clog_info!("ScriptCmd", "apply_script_commands: {} transform_writes", tw_count);
    }
    for tw in cmd_queue.transform_writes.drain(..) {
        renzora_core::clog_info!("ScriptCmd", "TW entity={:?} rot_delta={:?}", tw.entity, tw.rotation_delta);
        let Ok(mut t) = transforms.get_mut(tw.entity) else {
            renzora_core::clog_warn!("ScriptCmd", "Entity {:?} NOT FOUND in query!", tw.entity);
            continue;
        };

        if let Some(pos) = tw.new_position {
            t.translation = pos;
        }
        if let Some(rot) = tw.new_rotation {
            t.rotation = Quat::from_euler(
                EulerRot::XYZ,
                rot.x.to_radians(),
                rot.y.to_radians(),
                rot.z.to_radians(),
            );
        }
        if let Some(trans) = tw.translation {
            t.translation += trans;
        }
        if let Some(delta) = tw.rotation_delta {
            t.rotation *= Quat::from_euler(
                EulerRot::XYZ,
                delta.x.to_radians(),
                delta.y.to_radians(),
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
                    "warn" => renzora_core::clog_warn!("Script", "{}", message),
                    "error" => renzora_core::clog_error!("Script", "{}", message),
                    _ => renzora_core::clog_info!("Script", "{}", message),
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
                pending_env.sun_angles = Some((azimuth, elevation));
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
                renzora_core::clog_info!("Scene", "LoadScene requested: {}", path);
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

            // Commands that need additional systems (audio, physics, etc.)
            // are logged as unhandled for now — they'll be routed to
            // dedicated command queues as those systems are ported.
            other => {
                debug!("Unhandled script command: {:?}", other);
            }
        }
    }
}
