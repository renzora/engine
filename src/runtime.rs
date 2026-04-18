use bevy::prelude::*;

#[cfg(feature = "editor")]
pub use renzora_editor as renzora_shared;
#[cfg(not(feature = "editor"))]
pub use renzora_runtime as renzora_shared;

// ── Resolve path to renzora_runtime regardless of feature ────────────────
// Editor builds: renzora_editor::renzora_runtime
// Runtime/server builds: renzora_runtime directly
#[cfg(feature = "editor")]
use renzora_editor::renzora_runtime as rt;
#[cfg(not(feature = "editor"))]
use renzora_runtime as rt;

pub fn init_app() -> App {
    rt::init_app()
}

pub fn add_default_rendering(app: &mut App) {
    #[cfg(any(feature = "editor", not(feature = "server")))]
    rt::add_default_rendering(app);

    #[cfg(all(feature = "server", not(feature = "editor")))]
    {
        app.add_plugins(
            DefaultPlugins
                .set(bevy::window::WindowPlugin {
                    primary_window: None,
                    exit_condition: bevy::window::ExitCondition::DontExit,
                    ..default()
                })
        );
    }
}

pub fn add_engine_plugins(app: &mut App) {
    rt::add_engine_plugins(app);
}

/// Build the full runtime app (used by WASM start and server).
pub fn build_runtime_app() -> App {
    let mut app = init_app();
    add_default_rendering(&mut app);
    add_engine_plugins(&mut app);
    app
}
