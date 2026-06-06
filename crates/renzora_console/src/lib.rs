//! Console panel crate for the Renzora editor.

pub mod native;
pub mod state;

pub use state::*;

use bevy::prelude::*;

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Drain the global shared buffer into ConsoleState each frame.
fn drain_log_buffer(mut console: ResMut<ConsoleState>, time: Res<Time>, mut frame: Local<u64>) {
    *frame += 1;
    console.drain_shared_buffer(time.elapsed_secs_f64(), *frame);
}

/// Drain script log messages into the console.
fn drain_script_logs(
    mut console: ResMut<ConsoleState>,
    mut log_buffer: ResMut<renzora_scripting::systems::ScriptLogBuffer>,
) {
    for entry in log_buffer.entries.drain(..) {
        let level = match entry.level.as_str() {
            "warn" => LogLevel::Warning,
            "error" => LogLevel::Error,
            _ => LogLevel::Info,
        };
        console.log(level, "Script", entry.message);
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct ConsolePlugin;

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ConsolePlugin");
        app.insert_resource(ConsoleState::default());

        use renzora_editor::SplashState;
        app.add_systems(
            Update,
            (drain_log_buffer, drain_script_logs).run_if(in_state(SplashState::Editor)),
        );

        // Bevy-native (ember) console for the bevy_ui editor shell.
        native::register_native_console(app);
    }
}

renzora::add!(ConsolePlugin, Editor);
