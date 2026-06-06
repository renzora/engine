//! Gamepad debug panel — visualizes controller input (sticks, triggers, buttons).

pub mod native;
mod state;

use bevy::prelude::*;

use state::{update_gamepad_debug_state, GamepadDebugState};

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Plugin that registers the gamepad debug panel and its update system.
#[derive(Default)]
pub struct GamepadPlugin;

impl Plugin for GamepadPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] GamepadPlugin");
        app.init_resource::<GamepadDebugState>();
        use renzora_editor::SplashState;
        app.add_systems(
            Update,
            update_gamepad_debug_state.run_if(in_state(SplashState::Editor)),
        );
        // Bevy-native (ember) gamepad panel for the bevy_ui shell.
        native::register_native_gamepad(app);
    }
}

renzora::add!(GamepadPlugin, Editor);
