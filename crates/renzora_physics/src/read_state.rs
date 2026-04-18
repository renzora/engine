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
        commands.entity(entity).try_insert(PhysicsReadState::default());
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
