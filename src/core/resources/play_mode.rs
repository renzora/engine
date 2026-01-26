//! Play mode state for testing games in the editor

use bevy::prelude::*;

/// Current play mode state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayState {
    /// Normal editor mode
    #[default]
    Editing,
    /// Game is playing
    Playing,
    /// Game is paused
    Paused,
}

/// Resource tracking play mode state
#[derive(Resource, Default)]
pub struct PlayModeState {
    /// Current play state
    pub state: PlayState,
    /// Entity that has the active game camera (so we can remove it when stopping)
    pub active_game_camera: Option<Entity>,
    /// Whether to request entering play mode this frame
    pub request_play: bool,
    /// Whether to request stopping this frame
    pub request_stop: bool,
}

impl PlayModeState {
    pub fn is_playing(&self) -> bool {
        matches!(self.state, PlayState::Playing)
    }

    pub fn is_paused(&self) -> bool {
        matches!(self.state, PlayState::Paused)
    }

    pub fn is_editing(&self) -> bool {
        matches!(self.state, PlayState::Editing)
    }

    pub fn is_in_play_mode(&self) -> bool {
        !self.is_editing()
    }
}

/// Marker component for the play mode camera
#[derive(Component)]
pub struct PlayModeCamera;
