use bevy::prelude::*;

/// Application state for managing splash screen vs editor
#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Splash,
    Editor,
}
