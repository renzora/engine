//! Collision event collection system
//!
//! Collects physics collision events from Avian and stores them for scripts.

use bevy::prelude::*;
use bevy::ecs::message::MessageReader;

use avian3d::prelude::*;

use crate::core::PlayModeState;
use crate::scripting::resources::collisions::{CollisionEvent, ScriptCollisionEvents};

/// System to collect collision events from Avian physics
pub fn collect_collision_events(
    play_mode: Res<PlayModeState>,
    mut collision_events: ResMut<ScriptCollisionEvents>,
    mut collision_started: MessageReader<CollisionStart>,
    mut collision_ended: MessageReader<CollisionEnd>,
) {
    // Only collect events during play mode
    if !play_mode.is_scripts_running() {
        return;
    }

    // Clear previous frame's events
    collision_events.clear_frame_events();

    // Process collision started events
    // Avian uses body1/body2 which are Option<Entity> for rigid body entities
    for event in collision_started.read() {
        // Use body entities (rigid bodies) if available, otherwise use collider entities
        let entity1 = event.body1.unwrap_or(event.collider1);
        let entity2 = event.body2.unwrap_or(event.collider2);

        // Add event for both entities involved
        collision_events.collisions_started.push(CollisionEvent {
            entity: entity1,
            other_entity: entity2,
        });
        collision_events.collisions_started.push(CollisionEvent {
            entity: entity2,
            other_entity: entity1,
        });

        // Update active collisions
        collision_events.active_collisions
            .entry(entity1)
            .or_insert_with(Vec::new)
            .push(entity2);
        collision_events.active_collisions
            .entry(entity2)
            .or_insert_with(Vec::new)
            .push(entity1);
    }

    // Process collision ended events
    for event in collision_ended.read() {
        let entity1 = event.body1.unwrap_or(event.collider1);
        let entity2 = event.body2.unwrap_or(event.collider2);

        // Add event for both entities involved
        collision_events.collisions_ended.push(CollisionEvent {
            entity: entity1,
            other_entity: entity2,
        });
        collision_events.collisions_ended.push(CollisionEvent {
            entity: entity2,
            other_entity: entity1,
        });

        // Update active collisions
        if let Some(collisions) = collision_events.active_collisions.get_mut(&entity1) {
            collisions.retain(|e| *e != entity2);
        }
        if let Some(collisions) = collision_events.active_collisions.get_mut(&entity2) {
            collisions.retain(|e| *e != entity1);
        }
    }
}

/// System to clear collision data when play mode stops
pub fn clear_collisions_on_stop(
    play_mode: Res<PlayModeState>,
    mut collision_events: ResMut<ScriptCollisionEvents>,
    mut last_playing: Local<bool>,
) {
    let currently_playing = play_mode.is_in_play_mode();

    // Detect transition from playing to editing
    if *last_playing && !currently_playing {
        collision_events.clear_all();
    }

    *last_playing = currently_playing;
}
