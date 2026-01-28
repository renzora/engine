//! Physics command queue for scripts
//!
//! Collects physics commands from scripts to be processed by a separate system
//! that has access to Avian physics components.

use bevy::prelude::*;

/// A queued physics command from a script
#[derive(Clone, Debug)]
pub enum PhysicsCommand {
    ApplyForce { entity: Entity, force: Vec3 },
    ApplyImpulse { entity: Entity, impulse: Vec3 },
    ApplyTorque { entity: Entity, torque: Vec3 },
    SetVelocity { entity: Entity, velocity: Vec3 },
    SetAngularVelocity { entity: Entity, velocity: Vec3 },
    SetGravityScale { entity: Entity, scale: f32 },
    Raycast {
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        /// Entity that requested the raycast (for result storage)
        requester: Entity,
        /// Variable name to store result
        result_var: String,
    },
}

/// Resource to queue physics commands from scripts
#[derive(Resource, Default)]
pub struct PhysicsCommandQueue {
    pub commands: Vec<PhysicsCommand>,
}

impl PhysicsCommandQueue {
    pub fn push(&mut self, cmd: PhysicsCommand) {
        self.commands.push(cmd);
    }

    pub fn drain(&mut self) -> Vec<PhysicsCommand> {
        std::mem::take(&mut self.commands)
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

/// Resource to store raycast results for scripts
#[derive(Resource, Default)]
pub struct RaycastResults {
    /// Map of (entity, variable_name) -> RaycastHit
    pub results: std::collections::HashMap<(Entity, String), RaycastHit>,
}

/// Result of a raycast query
#[derive(Clone, Debug)]
pub struct RaycastHit {
    /// Whether the raycast hit something
    pub hit: bool,
    /// The entity that was hit (if any)
    pub entity: Option<Entity>,
    /// Hit point in world space
    pub point: Vec3,
    /// Surface normal at hit point
    pub normal: Vec3,
    /// Distance from origin to hit point
    pub distance: f32,
}

impl Default for RaycastHit {
    fn default() -> Self {
        Self {
            hit: false,
            entity: None,
            point: Vec3::ZERO,
            normal: Vec3::ZERO,
            distance: 0.0,
        }
    }
}
