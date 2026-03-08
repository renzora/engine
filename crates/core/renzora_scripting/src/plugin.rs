use bevy::prelude::*;
use std::path::PathBuf;

use crate::engine::ScriptEngine;
use crate::input::{ScriptInput, update_script_input};
use crate::resources::ScriptTimers;
use crate::resources::update_script_timers;

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

/// Scripting plugin for runtime (standalone games, no editor).
///
/// Registers Lua (default) and optionally Rhai backends, sets up
/// input collection, timer ticking, and system set ordering.
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
        engine.add_backend(Box::new(crate::backends::rhai_backend::RhaiBackend::new()));

        if let Some(ref folder) = self.scripts_folder {
            engine.set_scripts_folder(folder.clone());
        }

        app.insert_resource(engine)
            .init_resource::<ScriptInput>()
            .init_resource::<ScriptTimers>()
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
            // Pre-script systems
            .add_systems(
                Update,
                (
                    update_script_input,
                    update_script_timers,
                ).in_set(ScriptingSet::PreScript),
            );
    }
}
