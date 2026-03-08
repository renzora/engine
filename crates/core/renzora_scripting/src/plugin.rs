use bevy::prelude::*;
use std::path::PathBuf;

use crate::component::ScriptComponent;
use crate::engine::ScriptEngine;
use crate::input::{ScriptInput, update_script_input};
use crate::resources::ScriptTimers;
use crate::resources::update_script_timers;
use crate::systems::execution::{ScriptCommandQueue, ScriptEnvironmentCommands, ScriptLogBuffer, ScriptReflectionQueue};

/// System sets for ordering scripting systems
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ScriptingSet {
    /// Pre-script systems (input, timers)
    PreScript,
    /// Script execution
    ScriptExecution,
    /// Post-script command processing
    CommandProcessing,
    /// Debug draw
    DebugDraw,
    /// Cleanup
    Cleanup,
}

/// Scripting plugin — registers backends, input collection, script execution,
/// and command processing systems.
pub struct ScriptingPlugin {
    /// Path to the scripts folder
    pub scripts_folder: Option<PathBuf>,
}

impl ScriptingPlugin {
    pub fn new() -> Self {
        Self { scripts_folder: None }
    }

    pub fn with_scripts_folder(mut self, path: impl Into<PathBuf>) -> Self {
        self.scripts_folder = Some(path.into());
        self
    }
}

impl Default for ScriptingPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ScriptingPlugin {
    fn build(&self, app: &mut App) {
        // Create the script engine with available backends
        let mut engine = ScriptEngine::new();

        #[cfg(feature = "lua")]
        engine.add_backend(Box::new(crate::backends::lua::LuaBackend::new()));

        #[cfg(feature = "rhai")]
        engine.add_backend(Box::new(crate::backends::rhai::RhaiBackend::new()));

        if let Some(ref folder) = self.scripts_folder {
            engine.set_scripts_folder(folder.clone());
        }

        app.insert_resource(engine)
            .init_resource::<ScriptInput>()
            .init_resource::<ScriptTimers>()
            .init_resource::<ScriptCommandQueue>()
            .init_resource::<ScriptLogBuffer>()
            .init_resource::<ScriptEnvironmentCommands>()
            .init_resource::<ScriptReflectionQueue>()
            .register_type::<ScriptComponent>()
            // Configure system set ordering
            .configure_sets(
                Update,
                (
                    ScriptingSet::PreScript,
                    ScriptingSet::ScriptExecution,
                    ScriptingSet::CommandProcessing,
                    ScriptingSet::DebugDraw,
                    ScriptingSet::Cleanup,
                ).chain(),
            )
            // Pre-script systems (always run — input collection is cheap)
            .add_systems(
                Update,
                (
                    update_script_input,
                    update_script_timers,
                ).in_set(ScriptingSet::PreScript),
            )
            // Script execution — only when scripts should run
            .add_systems(
                Update,
                crate::systems::run_scripts
                    .in_set(ScriptingSet::ScriptExecution)
                    .run_if(scripts_should_run),
            )
            // Command processing — only when scripts should run
            .add_systems(
                Update,
                crate::systems::apply_script_commands
                    .in_set(ScriptingSet::CommandProcessing)
                    .run_if(scripts_should_run),
            )
            // Reflection-based component writes — exclusive system, runs after commands
            .add_systems(
                Update,
                crate::systems::apply_reflection_sets
                    .after(ScriptingSet::CommandProcessing)
                    .run_if(scripts_should_run),
            )
            // Sync scripts folder from CurrentProject
            .add_systems(
                Update,
                sync_scripts_folder.in_set(ScriptingSet::PreScript),
            );
    }
}

/// Run condition: scripts should execute this frame.
///
/// In editor: only when PlayModeState says scripts are running.
/// In standalone runtime (no PlayModeState): always run.
fn scripts_should_run(
    play_mode: Option<Res<renzora_core::PlayModeState>>,
) -> bool {
    match play_mode {
        Some(pm) => pm.is_scripts_running(),
        None => true, // standalone runtime — always run
    }
}

/// Tracks whether we've already synced the scripts folder for the current project.
#[derive(Resource, Default)]
struct ScriptsFolderSynced(Option<PathBuf>);

/// System that sets the scripts folder on the engine when a project is loaded.
fn sync_scripts_folder(
    project: Option<Res<renzora_core::CurrentProject>>,
    mut engine: ResMut<ScriptEngine>,
    mut synced: Local<Option<PathBuf>>,
) {
    let current_path = project.as_ref().map(|p| p.path.clone());
    if *synced == current_path {
        return; // already synced
    }
    *synced = current_path.clone();
    if let Some(path) = current_path {
        info!("[Scripting] Scripts folder set to: {:?}", path);
        engine.set_scripts_folder(path);
    }
}
