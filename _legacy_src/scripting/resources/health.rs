//! Health command queue for scripts
//!
//! Collects health-related commands from scripts to be processed by a separate system
//! that has access to HealthData components.

use bevy::prelude::*;

/// A queued health command from a script
#[derive(Clone, Debug)]
pub enum HealthCommand {
    /// Set health to a specific value
    SetHealth {
        entity: Entity,
        value: f32,
    },
    /// Set max health to a specific value
    SetMaxHealth {
        entity: Entity,
        value: f32,
    },
    /// Apply damage (reduces current health)
    Damage {
        entity: Entity,
        amount: f32,
    },
    /// Apply healing (increases current health up to max)
    Heal {
        entity: Entity,
        amount: f32,
    },
    /// Set invincibility state
    SetInvincible {
        entity: Entity,
        invincible: bool,
        /// Optional duration in seconds (0 = permanent)
        duration: f32,
    },
    /// Kill entity (set health to 0)
    Kill {
        entity: Entity,
    },
    /// Revive entity (restore to max health)
    Revive {
        entity: Entity,
    },
}

/// Resource to queue health commands from scripts
#[derive(Resource, Default)]
pub struct HealthCommandQueue {
    pub commands: Vec<HealthCommand>,
}

impl HealthCommandQueue {
    pub fn push(&mut self, cmd: HealthCommand) {
        self.commands.push(cmd);
    }

    pub fn drain(&mut self) -> Vec<HealthCommand> {
        std::mem::take(&mut self.commands)
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}
