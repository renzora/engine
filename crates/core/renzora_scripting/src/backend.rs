use std::path::{Path, PathBuf};

use crate::command::ScriptCommand;
use crate::component::{ScriptVariableDefinition, ScriptVariables};
use crate::context::ScriptContext;

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
