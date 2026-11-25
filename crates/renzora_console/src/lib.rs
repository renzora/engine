//! Console panel crate for the Renzora editor.

pub mod panel;
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

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct ConsolePlugin;

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ConsolePlugin");
        app.insert_resource(ConsoleState::default());

        use renzora_project_api::SplashState;
        // Script `print_log` lines arrive through the shared buffer like any
        // other log. A second drain of a scripting-only buffer used to add
        // each of them again.
        app.add_systems(Update, drain_log_buffer.run_if(in_state(SplashState::Editor)));

        // The console panel for the bevy_ui editor shell.
        panel::register(app);
    }
}
