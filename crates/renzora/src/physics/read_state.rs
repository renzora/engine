//! Per-entity physics mirrors, refreshed each frame by the backend.
//!
//! [`PhysicsReadState`] and [`CollisionReadState`] hold a script- and
//! blueprint-readable snapshot of what the simulation did, so Lua's
//! `get("PhysicsReadState.grounded")` and a blueprint's `physics/is_grounded`
//! have an up-to-date value without querying avian directly.
//!
//! The components and the systems that fill them both live here. Reading a
//! velocity means naming avian's `LinearVelocity`, and there are two of those,
//! one per dimension — so each updater is gated on the backend it reads, and a
//! build with neither compiles only the components.

use std::collections::HashSet;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::physics::data::PhysicsBodyData;
#[cfg(any(feature = "avian3d", feature = "avian2d"))]
use crate::physics::data::RuntimePhysics2d;

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

/// Per-entity collision snapshot, refreshed each frame from the backend's
/// contact pairs. Reflect-registered so blueprint `event/on_collision_enter` /
/// `_exit` (and Lua `get("CollisionReadState.entered")`) can read it by name.
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
    prev: HashSet<Entity>,
}

impl CollisionReadState {
    /// Fold this frame's contact set in, computing the enter/exit edges.
    ///
    /// A method rather than a free function in the backend because `prev` is
    /// the one field here that is bookkeeping rather than an answer: exposing
    /// it would let a caller desynchronise the edges from the set they were
    /// derived from, and both backends need exactly this fold anyway.
    ///
    /// `name_of` is passed in rather than a `Query<&Name>` taken, so this stays
    /// free of any particular system's borrows.
    pub fn apply_contacts(&mut self, current: HashSet<Entity>, name_of: impl Fn(Entity) -> String) {
        let entered: Vec<Entity> = current.difference(&self.prev).copied().collect();
        let exited: Vec<Entity> = self.prev.difference(&current).copied().collect();
        self.colliding = !current.is_empty();
        self.entered = !entered.is_empty();
        self.exited = !exited.is_empty();
        self.entered_name = entered.first().copied().map(&name_of).unwrap_or_default();
        self.exited_name = exited.first().copied().map(&name_of).unwrap_or_default();
        self.prev = current;
    }
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

/// Refreshes `PhysicsReadState` from Avian's current state. 2D-backend bodies
/// are excluded — their avian2d twin below owns them (each backend has its own
/// `LinearVelocity` type, and this one would zero a 2D body's reading).
#[cfg(feature = "avian3d")]
pub fn update_physics_read_state(
    mut q: Query<
        (
            &mut PhysicsReadState,
            Option<&avian3d::prelude::LinearVelocity>,
        ),
        Without<RuntimePhysics2d>,
    >,
) {
    for (mut rs, lv) in &mut q {
        let v = lv.map(|lv| lv.0).unwrap_or(Vec3::ZERO);
        rs.velocity = v;
        rs.speed = v.length();
        // `grounded` + `ground_normal` are written by the `kinematic_slide`
        // drain system each time a slide runs.
    }
}

/// avian2d twin of [`update_physics_read_state`]: mirrors 2D velocity into the
/// same Vec3 fields (z = 0) so scripts read one shape either way.
#[cfg(feature = "avian2d")]
pub fn update_physics_read_state_2d(
    mut q: Query<
        (
            &mut PhysicsReadState,
            Option<&avian2d::prelude::LinearVelocity>,
        ),
        With<RuntimePhysics2d>,
    >,
) {
    for (mut rs, lv) in &mut q {
        let v = lv.map(|lv| lv.0.extend(0.0)).unwrap_or(Vec3::ZERO);
        rs.velocity = v;
        rs.speed = v.length();
    }
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
/// set against the previous frame's. 2D-backend bodies are excluded — the 3D
/// contact graph never contains them, so this would wipe their `prev` set every
/// frame and the 2D twin below would report a fresh "entered" forever.
#[cfg(feature = "avian3d")]
pub fn update_collision_read_state(
    mut q: Query<(Entity, &mut CollisionReadState), Without<RuntimePhysics2d>>,
    collisions: avian3d::prelude::Collisions,
    names: Query<&Name>,
) {
    for (entity, mut rs) in &mut q {
        let mut current: std::collections::HashSet<Entity> = std::collections::HashSet::new();
        for pair in collisions.collisions_with(entity) {
            let other = if pair.collider1 == entity {
                pair.collider2
            } else {
                pair.collider1
            };
            current.insert(other);
        }
        rs.apply_contacts(current, |e| name_of(&names, e));
    }
}

/// avian2d twin of [`update_collision_read_state`], reading the 2D contact graph.
#[cfg(feature = "avian2d")]
pub fn update_collision_read_state_2d(
    mut q: Query<(Entity, &mut CollisionReadState), With<RuntimePhysics2d>>,
    collisions: avian2d::prelude::Collisions,
    names: Query<&Name>,
) {
    for (entity, mut rs) in &mut q {
        let mut current: std::collections::HashSet<Entity> = std::collections::HashSet::new();
        for pair in collisions.collisions_with(entity) {
            let other = if pair.collider1 == entity {
                pair.collider2
            } else {
                pair.collider1
            };
            current.insert(other);
        }
        rs.apply_contacts(current, |e| name_of(&names, e));
    }
}

/// The `Name` of an entity, or `""` — what `CollisionReadState`'s two name
/// fields carry. Shared by both backends' updaters.
#[cfg(any(feature = "avian3d", feature = "avian2d"))]
fn name_of(names: &Query<&Name>, entity: Entity) -> String {
    names
        .get(entity)
        .map(|n| n.as_str().to_string())
        .unwrap_or_default()
}
