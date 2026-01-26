use bevy::prelude::*;

/// Tracks whether egui has keyboard focus (e.g., text input is active)
#[derive(Resource, Default)]
pub struct InputFocusState {
    /// True when egui wants keyboard input (text field focused, etc.)
    pub egui_wants_keyboard: bool,
}
