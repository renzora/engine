//! Script-readable mirror of the controller.
//!
//! Same shape and same reasoning as `renzora_physics::PhysicsReadState`: the
//! drive system writes a plain reflected component each frame, so Lua's
//! `get("ParkourReadState.can_vault")` and the blueprint reflection nodes read
//! it through the dispatcher that already exists. No new script verbs, no
//! per-language work, and a game can build its own HUD prompt ("press E to
//! climb") out of the same fields the controller decides on.
//!
//! Writes from scripts are pointless rather than forbidden — the drive system
//! overwrites every field every frame.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::ParkourController;

/// Snapshot of the parkour controller's state, refreshed each frame.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct ParkourReadState {
    /// Current state as a lowercase string: `grounded`, `airborne`,
    /// `vaulting`, `mantling`, `hanging`, `climbing`, `wall_running`,
    /// `swinging`. See `ParkourState::as_str`.
    pub state: String,
    /// The parkour event that fired this frame, if any (`jump`, `land`,
    /// `vault_start`, `grab`, …), otherwise empty. Set for exactly one frame,
    /// so polling it once per `on_update` catches each event once.
    pub event: String,
    pub grounded: bool,
    pub velocity: Vec3,
    /// Magnitude of `velocity`.
    pub speed: f32,

    /// True while a vault or mantle is playing. Games use this to stop feeding
    /// movement input and to hold the camera steady through the move.
    pub traversing: bool,
    pub hanging: bool,
    pub climbing: bool,
    pub wall_running: bool,
    pub swinging: bool,

    /// A vaultable obstacle is in front of the character right now — the flag
    /// to hang a "press to vault" prompt on.
    pub can_vault: bool,
    pub can_mantle: bool,
    /// A ledge is at hand height: grabbable if the character were airborne.
    pub can_grab: bool,
    pub near_ladder: bool,
    /// Height of the ledge ahead, above the character's feet (`0` if none).
    pub ledge_height: f32,
}

impl Default for ParkourReadState {
    fn default() -> Self {
        Self {
            // Matches `ParkourMotion`'s starting state rather than deriving an
            // empty string: a script that reads `state` on the frame the
            // character spawns — before the drive system has run once — should
            // get a real state name, not "".
            state: crate::state::ParkourState::default_start().as_str().to_string(),
            event: String::new(),
            grounded: false,
            velocity: Vec3::ZERO,
            speed: 0.0,
            traversing: false,
            hanging: false,
            climbing: false,
            wall_running: false,
            swinging: false,
            can_vault: false,
            can_mantle: false,
            can_grab: false,
            near_ladder: false,
            ledge_height: 0.0,
        }
    }
}

/// Give every [`ParkourController`] its mirror. Separate from the controller's
/// own runtime scratch so a scene that was saved with the mirror in it (it is
/// a reflected component, so it round-trips) still gets one when loaded into a
/// build where it wasn't.
pub fn auto_init_read_state(
    mut commands: Commands,
    q: Query<Entity, (With<ParkourController>, Without<ParkourReadState>)>,
) {
    for entity in &q {
        commands.entity(entity).try_insert(ParkourReadState::default());
    }
}
