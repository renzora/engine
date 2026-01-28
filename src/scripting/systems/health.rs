//! Health command processing system
//!
//! Processes queued health commands from scripts, modifying HealthData components.

use bevy::prelude::*;

use crate::component_system::components::HealthData;
use crate::scripting::resources::{HealthCommand, HealthCommandQueue};

/// System to process queued health commands
pub fn process_health_commands(
    mut queue: ResMut<HealthCommandQueue>,
    mut health_query: Query<&mut HealthData>,
    mut commands: Commands,
) {
    if queue.is_empty() {
        return;
    }

    for cmd in queue.drain() {
        match cmd {
            HealthCommand::SetHealth { entity, value } => {
                if let Ok(mut health) = health_query.get_mut(entity) {
                    // Clamp to 0..max_health
                    health.current_health = value.clamp(0.0, health.max_health);
                    debug!(
                        "Set health of {:?} to {} (max: {})",
                        entity, health.current_health, health.max_health
                    );

                    // Check for death
                    if health.current_health <= 0.0 && health.destroy_on_death {
                        info!("Entity {:?} died (destroy_on_death=true)", entity);
                        commands.entity(entity).despawn();
                    }
                } else {
                    warn!(
                        "SetHealth: entity {:?} has no HealthData component",
                        entity
                    );
                }
            }

            HealthCommand::SetMaxHealth { entity, value } => {
                if let Ok(mut health) = health_query.get_mut(entity) {
                    let old_max = health.max_health;
                    health.max_health = value.max(1.0); // Minimum 1 max health

                    // Optionally scale current health proportionally
                    if old_max > 0.0 {
                        let ratio = health.current_health / old_max;
                        health.current_health = (ratio * health.max_health).min(health.max_health);
                    }

                    debug!(
                        "Set max health of {:?} to {} (current: {})",
                        entity, health.max_health, health.current_health
                    );
                } else {
                    warn!(
                        "SetMaxHealth: entity {:?} has no HealthData component",
                        entity
                    );
                }
            }

            HealthCommand::Damage { entity, amount } => {
                if let Ok(mut health) = health_query.get_mut(entity) {
                    // Check invincibility
                    if health.invincible {
                        debug!(
                            "Damage blocked: entity {:?} is invincible",
                            entity
                        );
                        continue;
                    }

                    let old_health = health.current_health;
                    health.current_health = (health.current_health - amount).max(0.0);

                    debug!(
                        "Damaged {:?} for {} (health: {} -> {})",
                        entity, amount, old_health, health.current_health
                    );

                    // Check for death
                    if health.current_health <= 0.0 && old_health > 0.0 {
                        info!("Entity {:?} died from damage", entity);
                        if health.destroy_on_death {
                            commands.entity(entity).despawn();
                        }
                    }
                } else {
                    warn!(
                        "Damage: entity {:?} has no HealthData component",
                        entity
                    );
                }
            }

            HealthCommand::Heal { entity, amount } => {
                if let Ok(mut health) = health_query.get_mut(entity) {
                    let old_health = health.current_health;
                    health.current_health =
                        (health.current_health + amount).min(health.max_health);

                    debug!(
                        "Healed {:?} for {} (health: {} -> {})",
                        entity, amount, old_health, health.current_health
                    );
                } else {
                    warn!(
                        "Heal: entity {:?} has no HealthData component",
                        entity
                    );
                }
            }

            HealthCommand::SetInvincible { entity, invincible } => {
                if let Ok(mut health) = health_query.get_mut(entity) {
                    health.invincible = invincible;
                    debug!(
                        "Set invincible of {:?} to {}",
                        entity, invincible
                    );
                } else {
                    warn!(
                        "SetInvincible: entity {:?} has no HealthData component",
                        entity
                    );
                }
            }
        }
    }
}
