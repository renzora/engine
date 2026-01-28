//! Collision events resource for scripts
//!
//! Collects physics collision events and makes them available to scripts.

use bevy::prelude::*;
use std::collections::HashMap;

/// A collision event between two entities
#[derive(Clone, Debug)]
pub struct CollisionEvent {
    /// The entity that collided
    pub entity: Entity,
    /// The other entity involved in the collision
    pub other_entity: Entity,
}

/// Resource that stores collision events for scripts to access
#[derive(Resource, Default)]
pub struct ScriptCollisionEvents {
    /// Collisions that started this frame
    pub collisions_started: Vec<CollisionEvent>,
    /// Collisions that ended this frame
    pub collisions_ended: Vec<CollisionEvent>,
    /// Per-entity collision data (entity → list of entities it's colliding with)
    pub active_collisions: HashMap<Entity, Vec<Entity>>,
}

impl ScriptCollisionEvents {
    /// Clear collision events for the new frame
    pub fn clear_frame_events(&mut self) {
        self.collisions_started.clear();
        self.collisions_ended.clear();
    }

    /// Clear all collision data (for play mode transitions)
    pub fn clear_all(&mut self) {
        self.collisions_started.clear();
        self.collisions_ended.clear();
        self.active_collisions.clear();
    }

    /// Get collisions that started for a specific entity this frame
    pub fn get_collisions_entered(&self, entity: Entity) -> Vec<Entity> {
        self.collisions_started
            .iter()
            .filter(|e| e.entity == entity)
            .map(|e| e.other_entity)
            .collect()
    }

    /// Get collisions that ended for a specific entity this frame
    pub fn get_collisions_exited(&self, entity: Entity) -> Vec<Entity> {
        self.collisions_ended
            .iter()
            .filter(|e| e.entity == entity)
            .map(|e| e.other_entity)
            .collect()
    }

    /// Check if entity is currently colliding with anything
    pub fn is_colliding(&self, entity: Entity) -> bool {
        self.active_collisions.get(&entity).map(|v| !v.is_empty()).unwrap_or(false)
    }

    /// Get list of entities currently colliding with the given entity
    pub fn get_active_collisions(&self, entity: Entity) -> Vec<Entity> {
        self.active_collisions.get(&entity).cloned().unwrap_or_default()
    }
}
