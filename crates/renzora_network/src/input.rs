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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_input_default_is_neutral() {
        let i = PlayerInput::default();
        assert_eq!(i.movement, Vec2::ZERO);
        assert_eq!(i.look_delta, Vec2::ZERO);
        assert!(!i.jump);
        assert!(!i.action1);
        assert!(!i.action2);
    }

    #[test]
    fn player_input_round_trip() {
        let i = PlayerInput {
            movement: Vec2::new(0.5, -1.0),
            look_delta: Vec2::new(2.0, 3.0),
            jump: true,
            action1: false,
            action2: true,
        };
        let json = serde_json::to_string(&i).unwrap();
        let back: PlayerInput = serde_json::from_str(&json).unwrap();
        assert_eq!(i, back);
    }
}
