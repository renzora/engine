//! Per-entity physics mirror component.
//!
//! [`PhysicsReadState`] holds a script-/blueprint-readable snapshot of the current
//! physics state for each entity with [`PhysicsBodyData`]. It's populated each
//! frame so that Lua's `get("PhysicsReadState.grounded")` and blueprint
//! `physics/is_grounded` nodes have an up-to-date value without having to query
//! Avian directly.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::data::PhysicsBodyData;

/// Snapshot of per-entity physics state, refreshed each frame.
///
/// Read-only from scripts / blueprints — writes are ignored (the updater
/// overwrites every frame). Reflect-registered so the existing `get`/`set`
/// path dispatcher can access fields by name (e.g. `PhysicsReadState.grounded`).
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct PhysicsReadState {
    /// True if a downward shape cast found ground this frame (below `max_slope`).
    pub grounded: bool,
    /// Linear velocity (world space). For kinematic bodies, this is the last
    /// commanded velocity rather than a solver-integrated value.
    pub velocity: Vec3,
    /// Scalar magnitude of `velocity`.
    pub speed: f32,
    /// Contact normal from the most recent ground hit (or `Vec3::Y` if airborne).
    pub ground_normal: Vec3,
}

/// Auto-inserts `PhysicsReadState` on any entity that has `PhysicsBodyData`
/// but not yet a read-state component.
pub fn auto_init_physics_read_state(
    mut commands: Commands,
    q: Query<Entity, (With<PhysicsBodyData>, Without<PhysicsReadState>)>,
) {
    for entity in &q {
        commands
            .entity(entity)
            .try_insert(PhysicsReadState::default());
    }
}

/// Refreshes `PhysicsReadState` from Avian's current state.
#[cfg(feature = "avian")]
pub fn update_physics_read_state(
    mut q: Query<(
        &mut PhysicsReadState,
        Option<&avian3d::prelude::LinearVelocity>,
    )>,
) {
    for (mut rs, lv) in &mut q {
        let v = lv.map(|lv| lv.0).unwrap_or(Vec3::ZERO);
        rs.velocity = v;
        rs.speed = v.length();
        // `grounded` + `ground_normal` are written by the `kinematic_slide`
        // drain system each time a slide runs.
    }
}

/// Per-entity collision snapshot, refreshed each frame from Avian's contact
/// pairs. Reflect-registered so blueprint `event/on_collision_enter`/`_exit`
/// (and Lua `get("CollisionReadState.entered")`) can read it by name. This is
/// the engine's first real collision-event source — previously the scripting
/// `on_collision` hook was an unpopulated stub.
///
/// Only the *first* entity entered/exited this frame is surfaced by name (the
/// blueprint event has a single `other` output); `colliding` reflects whether
/// any contact is currently active.
#[derive(Component, Clone, Debug, Default, Reflect)]
#[reflect(Component, Default)]
pub struct CollisionReadState {
    /// True while at least one contact is active this frame.
    pub colliding: bool,
    /// True on the frame a new contact began.
    pub entered: bool,
    /// True on the frame a contact ended.
    pub exited: bool,
    /// Name of the first entity that started touching this frame ("" if none).
    pub entered_name: String,
    /// Name of the first entity that stopped touching this frame ("" if none).
    pub exited_name: String,
    /// Last frame's colliding set, used to diff enter/exit. Not reflected.
    #[reflect(ignore)]
    prev: std::collections::HashSet<Entity>,
}

/// Auto-inserts `CollisionReadState` on any entity with `PhysicsBodyData`.
pub fn auto_init_collision_read_state(
    mut commands: Commands,
    q: Query<Entity, (With<PhysicsBodyData>, Without<CollisionReadState>)>,
) {
    for entity in &q {
        commands
            .entity(entity)
            .try_insert(CollisionReadState::default());
    }
}

/// Refreshes `CollisionReadState` by diffing each entity's current Avian contact
/// set against the previous frame's.
#[cfg(feature = "avian")]
pub fn update_collision_read_state(
    mut q: Query<(Entity, &mut CollisionReadState)>,
    collisions: avian3d::prelude::Collisions,
    names: Query<&Name>,
) {
    use std::collections::HashSet;
    for (entity, mut rs) in &mut q {
        let mut current: HashSet<Entity> = HashSet::new();
        for pair in collisions.collisions_with(entity) {
            let other = if pair.collider1 == entity {
                pair.collider2
            } else {
                pair.collider1
            };
            current.insert(other);
        }
        let name_of = |e: Option<&Entity>| {
            e.and_then(|x| names.get(*x).ok())
                .map(|n| n.as_str().to_string())
                .unwrap_or_default()
        };
        let entered: Vec<Entity> = current.difference(&rs.prev).copied().collect();
        let exited: Vec<Entity> = rs.prev.difference(&current).copied().collect();
        rs.colliding = !current.is_empty();
        rs.entered = !entered.is_empty();
        rs.exited = !exited.is_empty();
        rs.entered_name = name_of(entered.first());
        rs.exited_name = name_of(exited.first());
        rs.prev = current;
    }
}
