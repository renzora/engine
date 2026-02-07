//! Particle system resources
//!
//! Manages queued particle commands from scripts.

use bevy::prelude::*;

/// A queued particle command from a script
#[derive(Clone, Debug)]
pub enum ParticleScriptCommand {
    /// Start/resume playing the particle effect
    Play { entity_id: u64 },
    /// Pause the particle effect
    Pause { entity_id: u64 },
    /// Stop and reset the particle effect
    Stop { entity_id: u64 },
    /// Reset the effect to initial state
    Reset { entity_id: u64 },
    /// Emit a burst of particles
    Burst { entity_id: u64, count: u32 },
    /// Set the spawn rate multiplier
    SetRate { entity_id: u64, multiplier: f32 },
    /// Set the particle size multiplier
    SetScale { entity_id: u64, multiplier: f32 },
    /// Set the time scale
    SetTimeScale { entity_id: u64, scale: f32 },
    /// Set the color tint
    SetTint { entity_id: u64, r: f32, g: f32, b: f32, a: f32 },
    /// Set a custom float variable
    SetVariableFloat { entity_id: u64, name: String, value: f32 },
    /// Set a custom color variable
    SetVariableColor { entity_id: u64, name: String, r: f32, g: f32, b: f32, a: f32 },
    /// Set a custom vec3 variable
    SetVariableVec3 { entity_id: u64, name: String, x: f32, y: f32, z: f32 },
    /// Move emitter and emit a burst at position
    EmitAt { entity_id: u64, x: f32, y: f32, z: f32, count: Option<u32> },
}

/// Resource to queue particle commands from scripts
#[derive(Resource, Default)]
pub struct ParticleScriptCommandQueue {
    pub commands: Vec<ParticleScriptCommand>,
}

impl ParticleScriptCommandQueue {
    pub fn push(&mut self, cmd: ParticleScriptCommand) {
        self.commands.push(cmd);
    }

    pub fn drain(&mut self) -> Vec<ParticleScriptCommand> {
        std::mem::take(&mut self.commands)
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}
