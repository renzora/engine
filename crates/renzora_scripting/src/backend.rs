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

    /// Execute the `on_rpc(name, args)` hook for a received networked RPC.
    /// Returns commands produced by the script.
    ///
    /// Default is a no-op so backends without RPC support compile unchanged;
    /// a backend overrides this to dispatch to its `on_rpc` handler.
    fn call_on_rpc(
        &self,
        path: &Path,
        rpc_name: &str,
        args: &std::collections::HashMap<String, renzora::ScriptActionValue>,
        from: u64,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String> {
        let _ = (path, rpc_name, args, from, ctx, vars);
        Ok(Vec::new())
    }

    /// Execute the `on_ui(name, args)` hook for a UI markup callback (a
    /// `bevy_hui` template event with no Rust binding). `entity_bits` is the
    /// firing node's `Entity::to_bits()`, passed through as the third arg.
    /// Returns commands produced by the script.
    ///
    /// Default is a no-op so backends without UI support compile unchanged.
    fn call_on_ui(
        &self,
        path: &Path,
        name: &str,
        args: &std::collections::HashMap<String, renzora::ScriptActionValue>,
        entity_bits: u64,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String> {
        let _ = (path, name, args, entity_bits, ctx, vars);
        Ok(Vec::new())
    }

    /// Execute the `on_http(callback, status, body)` hook for a completed HTTP
    /// request. Default is a no-op so backends without HTTP support compile.
    fn call_on_http(
        &self,
        path: &Path,
        callback: &str,
        status: u16,
        body: &str,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String> {
        let _ = (path, callback, status, body, ctx, vars);
        Ok(Vec::new())
    }

    /// Execute a player-lifecycle hook — `on_player_joined(id)` when `joined`,
    /// else `on_player_left(id)`. Server-side. Default is a no-op so backends
    /// without lifecycle support compile unchanged.
    fn call_on_player_event(
        &self,
        path: &Path,
        id: u64,
        joined: bool,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String> {
        let _ = (path, id, joined, ctx, vars);
        Ok(Vec::new())
    }

    /// Check if a script file has changed and needs reloading
    fn needs_reload(&self, path: &Path) -> bool;

    /// Force reload a script from disk
    fn reload(&self, path: &Path) -> Result<(), String>;

    /// Evaluate an arbitrary expression (for console/REPL)
    fn eval_expression(&self, expr: &str) -> Result<String, String>;
}
