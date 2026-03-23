//! Bevy systems for script execution and command processing.

pub mod execution;
mod commands;
pub mod reflection;

pub use execution::{run_scripts, ScriptEnvironmentCommands, ScriptLogBuffer, ScriptLogEntry, ScriptReflectionQueue};
pub use commands::apply_script_commands;
pub use reflection::{apply_reflection_sets, get_reflected_field};
