use bevy::prelude::*;
use std::path::PathBuf;

use crate::component::ScriptComponent;
use crate::engine::ScriptEngine;
use crate::input::{ScriptInput, update_script_input};
use crate::resources::ScriptTimers;
use crate::resources::update_script_timers;
use crate::command::CharacterCommandQueue;
use crate::systems::execution::{ScriptCommandQueue, ScriptEnvironmentCommands, ScriptLogBuffer, ScriptReflectionQueue};

/// Events emitted when scripts are hot-reloaded.
#[derive(Resource, Default)]
pub struct ScriptReloadEvents {
    pub reloaded: Vec<String>,
}

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
        info!("[runtime] ScriptingPlugin");
        // Create the script engine with available backends
        let mut engine = ScriptEngine::new();

        #[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
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
            .init_resource::<CharacterCommandQueue>()
            .init_resource::<ScriptLogBuffer>()
            .init_resource::<ScriptEnvironmentCommands>()
            .init_resource::<ScriptReflectionQueue>()
            .init_resource::<ScriptReloadEvents>()
            .init_resource::<crate::extension::ScriptExtensions>()
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
            )
            // Hot-reload: check for modified script files
            .add_systems(
                Update,
                check_script_hot_reload.in_set(ScriptingSet::PreScript),
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

/// Check all active scripts for file changes and reload if modified.
fn check_script_hot_reload(
    engine: Res<ScriptEngine>,
    mut scripts: Query<&mut ScriptComponent>,
    mut reload_events: ResMut<ScriptReloadEvents>,
    mut timer: Local<f32>,
    time: Res<Time>,
) {
    // Only check every 0.5 seconds to avoid hammering the filesystem
    *timer += time.delta_secs();
    if *timer < 0.5 {
        return;
    }
    *timer = 0.0;

    reload_events.reloaded.clear();

    for mut sc in scripts.iter_mut() {
        for entry in sc.scripts.iter_mut() {
            let Some(ref path) = entry.script_path else { continue };
            if !entry.enabled { continue; }

            if engine.needs_reload(path) {
                let display_name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                match engine.reload(path) {
                    Ok(_) => {
                        // Re-run on_ready by resetting initialized flag
                        entry.runtime_state.initialized = false;
                        entry.runtime_state.has_error = false;
                        reload_events.reloaded.push(display_name);
                        info!("[Scripting] Hot-reloaded: {}", path.display());
                    }
                    Err(e) => {
                        warn!("[Scripting] Hot-reload failed for {}: {}", path.display(), e);
                    }
                }
            }
        }
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
