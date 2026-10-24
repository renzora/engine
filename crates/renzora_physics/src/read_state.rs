//! Per-entity physics mirror component.
//!
//! [`PhysicsReadState`] holds a script-/blueprint-readable snapshot of the current
//! physics state for each entity with [`PhysicsBodyData`]. It's populated each
//! frame so that Lua's `get("PhysicsReadState.grounded")` and blueprint
//! `physics/is_grounded` nodes have an up-to-date value without having to query
//! the physics backend directly.
//!
//! Both are filled from what the backend writes ([`BodyVelocity`],
//! [`PhysicsContacts`]), so one system each serves the 2D and 3D worlds and
//! every backend.

use bevy::prelude::*;
use renzora::physics::{BodyVelocity, CollisionEnded, CollisionStarted, PhysicsContacts};
use serde::{Deserialize, Serialize};

use crate::data::PhysicsBodyData;

/// Snapshot of per-entity physics state, refreshed each frame.
///
/// Read-only from scripts / blueprints: writes are ignored (the updater
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

/// Auto-inserts `PhysicsReadState`, and the `BodyVelocity` the backend fills
/// it from, on any entity that has `PhysicsBodyData` but not yet a read-state
/// component.
pub fn auto_init_physics_read_state(
    mut commands: Commands,
    q: Query<Entity, (With<PhysicsBodyData>, Without<PhysicsReadState>)>,
) {
    for entity in &q {
        commands
            .entity(entity)
            .try_insert((PhysicsReadState::default(), BodyVelocity::default()));
    }
}

/// Refreshes `PhysicsReadState` from the backend's `BodyVelocity`. A 2D body's
/// velocity has `z = 0`, so scripts read one shape either way.
pub fn update_physics_read_state(
    mut q: Query<(&mut PhysicsReadState, Option<&BodyVelocity>)>,
) {
    for (mut rs, velocity) in &mut q {
        let v = velocity.map(|v| v.linear).unwrap_or(Vec3::ZERO);
        rs.velocity = v;
        rs.speed = v.length();
        // `grounded` + `ground_normal` are written by the kinematic slide each
        // time one runs.
    }
}

/// Per-entity collision snapshot, refreshed each frame from the backend's
/// contacts. Reflect-registered so blueprint `event/on_collision_enter`/`_exit`
/// (and Lua `get("CollisionReadState.entered")`) can read it by name. This is
/// the engine's first real collision-event source; previously the scripting
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

/// Auto-inserts `CollisionReadState`, and the `PhysicsContacts` the backend
/// fills it from, on any entity with `PhysicsBodyData`.
pub fn auto_init_collision_read_state(
    mut commands: Commands,
    q: Query<Entity, (With<PhysicsBodyData>, Without<CollisionReadState>)>,
) {
    for entity in &q {
        commands
            .entity(entity)
            .try_insert((CollisionReadState::default(), PhysicsContacts::default()));
    }
}

/// Refreshes `CollisionReadState` by diffing each entity's current contact set
/// against the previous frame's, and triggers [`CollisionStarted`] /
/// [`CollisionEnded`] for every pair that changed. Done here, once, so the
/// events mean the same thing whichever backend wrote the contacts.
pub fn update_collision_read_state(
    mut q: Query<(Entity, &mut CollisionReadState, Option<&PhysicsContacts>)>,
    names: Query<&Name>,
    mut commands: Commands,
) {
    for (entity, mut rs, contacts) in &mut q {
        let contacts = contacts.map(|c| c.0.as_slice()).unwrap_or_default();
        let mut current = std::collections::HashSet::with_capacity(contacts.len());
        for contact in contacts {
            // A pair can report more than one contact; it starts once.
            if current.insert(contact.entity) && !rs.prev.contains(&contact.entity) {
                commands.trigger(CollisionStarted { entity, other: contact.entity, contact: *contact });
            }
        }
        for &other in rs.prev.difference(&current) {
            commands.trigger(CollisionEnded { entity, other });
        }
        diff_collision_state(&mut rs, current, &names);
    }
}

fn diff_collision_state(
    rs: &mut CollisionReadState,
    current: std::collections::HashSet<Entity>,
    names: &Query<&Name>,
) {
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
