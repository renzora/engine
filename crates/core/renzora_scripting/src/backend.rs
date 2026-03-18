use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::command::ScriptCommand;
use crate::component::{ScriptVariableDefinition, ScriptVariables};
use crate::context::ScriptContext;

/// A function that can read a script file's contents by path.
/// Used to support reading scripts from rpak archives on Android/exported builds.
/// Returns `Some(source)` if the file was found, `None` otherwise.
pub type FileReader = Arc<dyn Fn(&Path) -> Option<String> + Send + Sync>;

/// Trait that script language backends must implement.
/// Each backend (Lua, Rhai, etc.) provides its own compilation, execution,
/// and variable marshalling logic while producing the same `ScriptCommand`s.
pub trait ScriptBackend: Send + Sync {
    /// Human-readable name (e.g. "Lua", "Rhai")
    fn name(&self) -> &str;

    /// File extensions this backend handles (e.g. &["lua"] or &["rhai"])
    fn extensions(&self) -> &[&str];

    /// Set the folder to scan for script files
    fn set_scripts_folder(&mut self, path: PathBuf);

    /// Set an optional file reader for VFS/rpak support.
    /// When set, backends should try this reader before falling back to `std::fs`.
    fn set_file_reader(&mut self, reader: FileReader);

    /// List available scripts as (display_name, path) pairs
    fn get_available_scripts(&self) -> Vec<(String, PathBuf)>;

    /// Get the props/variable definitions declared by a script file
    fn get_script_props(&self, path: &Path) -> Vec<ScriptVariableDefinition>;

    /// Execute the `on_ready` lifecycle hook.
    /// Returns commands produced by the script.
    fn call_on_ready(
        &self,
        path: &Path,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String>;

    /// Execute the `on_update` lifecycle hook.
    /// Returns commands produced by the script.
    fn call_on_update(
        &self,
        path: &Path,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String>;

    /// Check if a script file has changed and needs reloading
    fn needs_reload(&self, path: &Path) -> bool;

    /// Force reload a script from disk
    fn reload(&self, path: &Path) -> Result<(), String>;

    /// Evaluate an arbitrary expression (for console/REPL)
    fn eval_expression(&self, expr: &str) -> Result<String, String>;
}
