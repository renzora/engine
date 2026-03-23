//! Player input protocol for client-side prediction.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Inputs sent from client to server each tick.
///
/// Lightyear buffers these per-tick and handles packet loss
/// by resending the last N frames per packet.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PlayerInput {
    /// Movement direction (WASD / stick). Normalized by the client.
    pub movement: Vec2,
    /// Look delta (mouse movement / right stick).
    pub look_delta: Vec2,
    /// Jump requested this tick.
    pub jump: bool,
    /// Primary action (e.g. attack, interact).
    pub action1: bool,
    /// Secondary action (e.g. block, alt-fire).
    pub action2: bool,
}
