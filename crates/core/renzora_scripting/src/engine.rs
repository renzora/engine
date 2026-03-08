use bevy::prelude::*;
use std::path::{Path, PathBuf};

use crate::backend::ScriptBackend;

use crate::component::{ScriptVariableDefinition, ScriptVariables};
use crate::context::ScriptContext;

/// Resource holding the active script engine with swappable backends.
/// Multiple backends can be registered (e.g. Lua + Rhai) and scripts
/// are dispatched to the backend matching their file extension.
#[derive(Resource)]
pub struct ScriptEngine {
    backends: Vec<Box<dyn ScriptBackend>>,
    scripts_folder: Option<PathBuf>,
}

impl ScriptEngine {
    pub fn new() -> Self {
        Self { backends: Vec::new(), scripts_folder: None }
    }

    /// Register a language backend
    pub fn add_backend(&mut self, backend: Box<dyn ScriptBackend>) {
        log::info!("[Scripting] Registered {} backend (extensions: {:?})", backend.name(), backend.extensions());
        self.backends.push(backend);
    }

    /// Set scripts folder on all backends
    pub fn set_scripts_folder(&mut self, path: PathBuf) {
        self.scripts_folder = Some(path.clone());
        for b in &mut self.backends {
            b.set_scripts_folder(path.clone());
        }
    }

    /// Resolve a script path — if relative, prepend scripts_folder.
    fn resolve_path(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else if let Some(folder) = &self.scripts_folder {
            folder.join(path)
        } else {
            path.to_path_buf()
        }
    }

    /// Get all available scripts from all backends
    pub fn get_available_scripts(&self) -> Vec<(String, PathBuf)> {
        let mut scripts = Vec::new();
        for b in &self.backends {
            scripts.extend(b.get_available_scripts());
        }
        scripts.sort_by(|a, b| a.0.cmp(&b.0));
        scripts
    }

    /// Find the backend for a given file path
    fn backend_for(&self, path: &Path) -> Option<&dyn ScriptBackend> {
        let ext = path.extension()?.to_str()?;
        self.backends.iter()
            .find(|b| b.extensions().contains(&ext))
            .map(|b| b.as_ref())
    }

    /// Get props for a script
    pub fn get_script_props(&self, path: &Path) -> Vec<ScriptVariableDefinition> {
        let resolved = self.resolve_path(path);
        self.backend_for(path)
            .map(|b| b.get_script_props(&resolved))
            .unwrap_or_default()
    }

    pub fn call_on_ready(
        &self,
        path: &Path,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<(), String> {
        let resolved = self.resolve_path(path);
        let backend = self.backend_for(path)
            .ok_or_else(|| format!("No backend for {:?}", path.extension()))?;
        let commands = backend.call_on_ready(&resolved, ctx, vars)?;
        for cmd in commands {
            ctx.process_command(cmd);
        }
        Ok(())
    }

    pub fn call_on_update(
        &self,
        path: &Path,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<(), String> {
        let resolved = self.resolve_path(path);
        let backend = self.backend_for(path)
            .ok_or_else(|| format!("No backend for {:?}", path.extension()))?;
        let commands = backend.call_on_update(&resolved, ctx, vars)?;
        for cmd in commands {
            ctx.process_command(cmd);
        }
        Ok(())
    }

    pub fn needs_reload(&self, path: &Path) -> bool {
        let resolved = self.resolve_path(path);
        self.backend_for(path)
            .map(|b| b.needs_reload(&resolved))
            .unwrap_or(false)
    }

    pub fn reload(&self, path: &Path) -> Result<(), String> {
        let resolved = self.resolve_path(path);
        self.backend_for(path)
            .ok_or_else(|| format!("No backend for {:?}", path.extension()))?
            .reload(&resolved)
    }

    pub fn eval_expression(&self, expr: &str) -> Result<String, String> {
        // Try first backend (primary language)
        self.backends.first()
            .ok_or_else(|| "No backends registered".to_string())?
            .eval_expression(expr)
    }
}
