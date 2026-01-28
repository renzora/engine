mod api;
mod component;
mod registry;
mod rhai_engine;
mod rhai_context;
mod rhai_commands;
mod rhai_api;
mod runtime;
mod builtin_scripts;

pub use api::*;
pub use component::*;
pub use registry::*;
pub use rhai_engine::*;
pub use rhai_context::*;
pub use rhai_commands::*;
pub use runtime::*;

use bevy::prelude::*;
use crate::core::{AppState, PlayModeState, PlayState};
use crate::core::resources::console::{console_log, LogLevel};
use crate::project::CurrentProject;

pub struct ScriptingPlugin;

impl Plugin for ScriptingPlugin {
    fn build(&self, app: &mut App) {
        // Initialize registry with built-in scripts
        let mut registry = ScriptRegistry::new();
        builtin_scripts::register_builtin_scripts(&mut registry);

        app.insert_resource(registry)
            .insert_resource(RhaiScriptEngine::new())
            .init_resource::<ScriptInput>()
            .add_systems(
                Update,
                (
                    update_rhai_scripts_folder,
                    update_script_input,
                    reset_scripts_on_play_start,
                    run_scripts,
                    run_rhai_scripts,
                )
                    .chain()
                    .run_if(in_state(AppState::Editor)),
            );
    }
}

/// System to reset script runtime state when play mode starts
fn reset_scripts_on_play_start(
    play_mode: Res<PlayModeState>,
    mut scripts: Query<&mut ScriptComponent>,
    mut last_play_state: Local<PlayState>,
) {
    // Detect transition from Editing to Playing
    if *last_play_state != PlayState::Playing && play_mode.state == PlayState::Playing {
        let count = scripts.iter().count();
        if count > 0 {
            console_log(LogLevel::Info, "Script", format!("Play mode started - initializing {} script(s)", count));
        }
        for mut script in scripts.iter_mut() {
            script.runtime_state.initialized = false;
            script.runtime_state.has_error = false; // Reset errors on play start
        }
    }
    *last_play_state = play_mode.state;
}

/// System to update the Rhai scripts folder when project changes
fn update_rhai_scripts_folder(
    current_project: Option<Res<CurrentProject>>,
    mut rhai_engine: ResMut<RhaiScriptEngine>,
    mut last_project_path: Local<Option<std::path::PathBuf>>,
) {
    let current_path = current_project.as_ref().map(|p| p.path.clone());

    if *last_project_path != current_path {
        *last_project_path = current_path.clone();

        if let Some(project_path) = current_path {
            let scripts_folder = project_path.join("scripts");
            // Create scripts folder if it doesn't exist
            let _ = std::fs::create_dir_all(&scripts_folder);
            bevy::log::info!("[Scripting] Scripts folder set to: {:?}", scripts_folder);
            rhai_engine.set_scripts_folder(scripts_folder.clone());

            // Log available scripts
            let available = rhai_engine.get_available_scripts();
            bevy::log::info!("[Scripting] Found {} scripts: {:?}", available.len(), available.iter().map(|(n, _)| n).collect::<Vec<_>>());
        }
    }
}
