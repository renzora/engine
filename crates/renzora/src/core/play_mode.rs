//! Play / Simulate / Edit state, and the run conditions built on it.

use bevy::prelude::*;

/// Current play-mode state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PlayState {
    /// Normal editing.
    #[default]
    Editing,
    /// Game is running (game camera active, editor overlays hidden).
    Playing,
    /// Game is paused.
    Paused,
    /// Simulating in-editor: scripts + physics + animation run, but the editor
    /// stays fully live — editor camera, gizmos, selection and inspector all
    /// remain active, unlike [`PlayState::Playing`] which swaps to the game
    /// camera and hides the editor chrome. Entering snapshots the scene; Stop
    /// restores it, so a simulation never permanently mutates the scene.
    Simulating,
}

/// Editor signal: a viewport "brush"/paint tool is currently active (e.g. the
/// tilemap paint tool). While set, the 2D pick/drag systems stand down so a
/// click paints instead of re-selecting or dragging the entity out from under
/// the brush. Any editor tool may raise it; it lives in the contract so the
/// gizmo crate can read it without depending on the tool's crate.
#[derive(Resource, Default)]
pub struct ViewportBrushActive(pub bool);

/// Resource that tracks play mode state and pending transitions.
#[derive(Resource, Default)]
pub struct PlayModeState {
    pub state: PlayState,
    /// Entity of the active game camera during play mode.
    pub active_game_camera: Option<bevy::ecs::entity::Entity>,
    /// Set to `true` to request entering play mode next frame.
    pub request_play: bool,
    /// Set to `true` to request entering Simulate mode next frame (run the
    /// simulation while keeping the editor live; see [`PlayState::Simulating`]).
    pub request_simulate: bool,
    /// Set to `true` to request stopping play mode next frame.
    pub request_stop: bool,
    /// Set to `true` to toggle pause.
    pub request_pause: bool,
}

impl PlayModeState {
    pub fn is_playing(&self) -> bool {
        self.state == PlayState::Playing
    }
    pub fn is_paused(&self) -> bool {
        self.state == PlayState::Paused
    }
    pub fn is_editing(&self) -> bool {
        self.state == PlayState::Editing
    }
    /// Returns true while simulating in-editor (editor chrome stays live).
    pub fn is_simulating(&self) -> bool {
        self.state == PlayState::Simulating
    }
    /// Returns true if in Playing or Paused state (full play mode). Deliberately
    /// EXCLUDES `Simulating`: callers use this to hide editor chrome / swap to the
    /// game camera, and Simulate keeps the editor live, so it must read as "not in
    /// play mode" for all that tooling to stay active.
    pub fn is_in_play_mode(&self) -> bool {
        matches!(self.state, PlayState::Playing | PlayState::Paused)
    }
    /// Returns true if scripts (and the physics/animation they drive) should be
    /// executing this frame — true in both full Play and in-editor Simulate.
    pub fn is_scripts_running(&self) -> bool {
        matches!(self.state, PlayState::Playing | PlayState::Simulating)
    }
}

/// Run condition: returns true when NOT in play mode (i.e. editing).
/// Use as `.run_if(not_in_play_mode)` on editor systems that should be disabled during play.
pub fn not_in_play_mode(play_mode: Option<Res<PlayModeState>>) -> bool {
    !play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode())
}

/// Run condition: returns true when the viewport is in 3D view. Use on
/// editor systems whose visuals (transform gizmo arrows, collider wireframes,
/// rotation pies, etc.) only make sense projecting through a 3D camera.
pub fn in_three_view(settings: Option<Res<crate::core::viewport_types::ViewportSettings>>) -> bool {
    use crate::core::viewport_types::ViewportView;
    settings.is_none_or(|s| s.viewport_view == ViewportView::Three)
}

/// Run condition: returns true when the viewport is in 2D view. Use on
/// editor systems that pick/drag/draw 2D entities through the orthographic
/// editor camera.
pub fn in_two_view(settings: Option<Res<crate::core::viewport_types::ViewportSettings>>) -> bool {
    use crate::core::viewport_types::ViewportView;
    settings.is_some_and(|s| s.viewport_view == ViewportView::Two)
}

/// Marker component added to the game camera entity during play mode.
#[derive(Component)]
pub struct PlayModeCamera;
