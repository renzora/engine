//! Particle command processing system
//!
//! Processes particle commands queued by scripts.

use bevy::prelude::*;
use bevy_hanabi::prelude::*;

use crate::particles::{HanabiEffectData, EffectVariable};
use crate::scripting::resources::ParticleScriptCommandQueue;
use super::super::resources::ParticleScriptCommand;

/// System to process particle commands from scripts
pub fn process_particle_script_commands(
    mut commands: ResMut<ParticleScriptCommandQueue>,
    mut effect_query: Query<(Entity, &mut HanabiEffectData, Option<&mut EffectSpawner>, Option<&mut Transform>)>,
) {
    for cmd in commands.drain() {
        let entity_id = match &cmd {
            ParticleScriptCommand::Play { entity_id } => *entity_id,
            ParticleScriptCommand::Pause { entity_id } => *entity_id,
            ParticleScriptCommand::Stop { entity_id } => *entity_id,
            ParticleScriptCommand::Reset { entity_id } => *entity_id,
            ParticleScriptCommand::Burst { entity_id, .. } => *entity_id,
            ParticleScriptCommand::SetRate { entity_id, .. } => *entity_id,
            ParticleScriptCommand::SetScale { entity_id, .. } => *entity_id,
            ParticleScriptCommand::SetTimeScale { entity_id, .. } => *entity_id,
            ParticleScriptCommand::SetTint { entity_id, .. } => *entity_id,
            ParticleScriptCommand::SetVariableFloat { entity_id, .. } => *entity_id,
            ParticleScriptCommand::SetVariableColor { entity_id, .. } => *entity_id,
            ParticleScriptCommand::SetVariableVec3 { entity_id, .. } => *entity_id,
            ParticleScriptCommand::EmitAt { entity_id, .. } => *entity_id,
        };

        // Try to find the entity with this ID
        // Note: In practice, scripts would use the entity_id from ctx.entity
        let target_entity = Entity::from_bits(entity_id);

        // Find the matching entity in our query
        let found = effect_query
            .iter_mut()
            .find(|(e, _, _, _)| *e == target_entity);

        let Some((_, mut effect_data, spawner_opt, transform_opt)) = found else {
            warn!("Particle command for unknown entity: {}", entity_id);
            continue;
        };

        match cmd {
            ParticleScriptCommand::Play { .. } => {
                effect_data.playing = true;
                if let Some(mut spawner) = spawner_opt {
                    spawner.active = true;
                }
            }
            ParticleScriptCommand::Pause { .. } => {
                effect_data.playing = false;
                if let Some(mut spawner) = spawner_opt {
                    spawner.active = false;
                }
            }
            ParticleScriptCommand::Stop { .. } => {
                effect_data.playing = false;
                if let Some(mut spawner) = spawner_opt {
                    spawner.active = false;
                    spawner.reset();
                }
            }
            ParticleScriptCommand::Reset { .. } => {
                if let Some(mut spawner) = spawner_opt {
                    spawner.reset();
                }
            }
            ParticleScriptCommand::Burst { count, .. } => {
                // For burst spawning, we'd need to modify the spawner
                // This is a simplification - just reset which may trigger a burst
                if let Some(mut spawner) = spawner_opt {
                    spawner.reset();
                }
            }
            ParticleScriptCommand::SetRate { multiplier, .. } => {
                effect_data.rate_multiplier = multiplier;
            }
            ParticleScriptCommand::SetScale { multiplier, .. } => {
                effect_data.scale_multiplier = multiplier;
            }
            ParticleScriptCommand::SetTimeScale { scale, .. } => {
                effect_data.time_scale = scale;
            }
            ParticleScriptCommand::SetTint { r, g, b, a, .. } => {
                effect_data.color_tint = [r, g, b, a];
            }
            ParticleScriptCommand::SetVariableFloat { name, value, .. } => {
                effect_data.variable_overrides.insert(name, EffectVariable::Float {
                    value,
                    min: 0.0,
                    max: 1.0,
                });
            }
            ParticleScriptCommand::SetVariableColor { name, r, g, b, a, .. } => {
                effect_data.variable_overrides.insert(name, EffectVariable::Color {
                    value: [r, g, b, a],
                });
            }
            ParticleScriptCommand::SetVariableVec3 { name, x, y, z, .. } => {
                effect_data.variable_overrides.insert(name, EffectVariable::Vec3 {
                    value: [x, y, z],
                });
            }
            ParticleScriptCommand::EmitAt { x, y, z, count, .. } => {
                // Move the entity to the position
                if let Some(mut transform) = transform_opt {
                    transform.translation = Vec3::new(x, y, z);
                }
                // Trigger a reset/burst
                if let Some(mut spawner) = spawner_opt {
                    spawner.reset();
                }
            }
        }
    }
}
