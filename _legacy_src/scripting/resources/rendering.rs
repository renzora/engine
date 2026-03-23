//! Rendering command queue for scripts
//!
//! Collects rendering commands from scripts to be processed by a separate system
//! that has access to material and light components.

use bevy::prelude::*;

/// A queued rendering command from a script
#[derive(Clone, Debug)]
pub enum RenderingCommand {
    SetMaterialColor {
        entity: Entity,
        color: [f32; 4],
    },
    SetLightIntensity {
        entity: Entity,
        intensity: f32,
    },
    SetLightColor {
        entity: Entity,
        color: [f32; 3],
    },
}

/// Resource to queue rendering commands from scripts
#[derive(Resource, Default)]
pub struct RenderingCommandQueue {
    pub commands: Vec<RenderingCommand>,
}

impl RenderingCommandQueue {
    pub fn push(&mut self, cmd: RenderingCommand) {
        self.commands.push(cmd);
    }

    pub fn drain(&mut self) -> Vec<RenderingCommand> {
        std::mem::take(&mut self.commands)
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}
