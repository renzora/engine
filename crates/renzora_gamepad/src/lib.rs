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
        use renzora::SplashState;
        app.add_systems(
            Update,
            (update_gamepad_debug_state, hide_gamepad_entities)
                .run_if(in_state(SplashState::Editor)),
        );
        // Bevy-native (ember) gamepad panel for the bevy_ui shell.
        native::register_native_gamepad(app);
    }
}

/// Bevy spawns one entity (with a `Name`) per connected gamepad, which would
/// otherwise appear as a loose entity in the scene hierarchy — and get picked up
/// by the scene saver. Tag each with `HideInHierarchy` so it's treated as
/// editor-internal, the same as the VR controller wands. Runs continuously so a
/// pad plugged in mid-session is caught; `Without` makes it a no-op once tagged.
fn hide_gamepad_entities(
    mut commands: Commands,
    pads: Query<
        Entity,
        (
            With<bevy::input::gamepad::Gamepad>,
            Without<renzora::HideInHierarchy>,
        ),
    >,
) {
    for e in &pads {
        commands.entity(e).try_insert(renzora::HideInHierarchy);
    }
}

renzora::add!(GamepadPlugin, Editor);
