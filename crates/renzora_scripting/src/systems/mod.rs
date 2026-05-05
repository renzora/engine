//! Bevy systems for script execution and command processing.

mod commands;
pub mod execution;
pub mod reflection;

pub use commands::apply_script_commands;
pub use execution::{
    run_scripts, ScriptEnvironmentCommands, ScriptLogBuffer, ScriptLogEntry, ScriptReflectionQueue,
};
pub use reflection::{apply_reflection_sets, get_reflected_field};
