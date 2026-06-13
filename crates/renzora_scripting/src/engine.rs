use bevy::prelude::*;
use std::path::{Path, PathBuf};

use crate::backend::{FileReader, ScriptBackend};

use crate::component::{ScriptVariableDefinition, ScriptVariables};
use crate::context::ScriptContext;

/// Resource holding the active script engine with swappable backends.
/// Multiple backends can be registered (e.g. Lua + Rhai) and scripts
/// are dispatched to the backend matching their file extension.
#[derive(Resource)]
pub struct ScriptEngine {
    backends: Vec<Box<dyn ScriptBackend>>,
    scripts_folder: Option<PathBuf>,
    file_reader: Option<FileReader>,
}

impl Default for ScriptEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptEngine {
    pub fn new() -> Self {
        Self {
            backends: Vec::new(),
            scripts_folder: None,
            file_reader: None,
        }
    }

    /// Number of registered script backends. Exposed for diagnostic
    /// panels (renzora_debugger scripting panel) so they can show the
    /// scripting subsystem's wiring state without poking internals.
    pub fn backend_count(&self) -> usize {
        self.backends.len()
    }

    /// Currently configured scripts root folder, if one's been set.
    /// Editor diagnostic panels read this to surface "where scripts
    /// are being loaded from" without taking a mutable borrow.
    pub fn scripts_folder(&self) -> Option<&std::path::Path> {
        self.scripts_folder.as_deref()
    }

    /// Register a language backend
    pub fn add_backend(&mut self, backend: Box<dyn ScriptBackend>) {
        log::info!(
            "[Scripting] Registered {} backend (extensions: {:?})",
            backend.name(),
            backend.extensions()
        );
        self.backends.push(backend);
    }

    /// Set scripts folder on all backends
    pub fn set_scripts_folder(&mut self, path: PathBuf) {
        self.scripts_folder = Some(path.clone());
        for b in &mut self.backends {
            b.set_scripts_folder(path.clone());
        }
    }

    /// Set a file reader for VFS/rpak support on all backends.
    pub fn set_file_reader(&mut self, reader: FileReader) {
        self.file_reader = Some(reader.clone());
        for b in &mut self.backends {
            b.set_file_reader(reader.clone());
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
        self.backends
            .iter()
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
        let backend = self
            .backend_for(path)
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
        let backend = self
            .backend_for(path)
            .ok_or_else(|| format!("No backend for {:?}", path.extension()))?;
        let commands = backend.call_on_update(&resolved, ctx, vars)?;
        for cmd in commands {
            ctx.process_command(cmd);
        }
        Ok(())
    }

    /// Dispatch a received networked RPC to a script's `on_rpc(name, args)`.
    pub fn call_on_rpc(
        &self,
        path: &Path,
        rpc_name: &str,
        args: &std::collections::HashMap<String, renzora::ScriptActionValue>,
        from: u64,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<(), String> {
        let resolved = self.resolve_path(path);
        let backend = self
            .backend_for(path)
            .ok_or_else(|| format!("No backend for {:?}", path.extension()))?;
        let commands = backend.call_on_rpc(&resolved, rpc_name, args, from, ctx, vars)?;
        for cmd in commands {
            ctx.process_command(cmd);
        }
        Ok(())
    }

    /// Dispatch a UI markup callback to a script's `on_ui(name, args)` hook.
    pub fn call_on_ui(
        &self,
        path: &Path,
        name: &str,
        args: &std::collections::HashMap<String, renzora::ScriptActionValue>,
        entity_bits: u64,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<(), String> {
        let resolved = self.resolve_path(path);
        let backend = self
            .backend_for(path)
            .ok_or_else(|| format!("No backend for {:?}", path.extension()))?;
        let commands = backend.call_on_ui(&resolved, name, args, entity_bits, ctx, vars)?;
        for cmd in commands {
            ctx.process_command(cmd);
        }
        Ok(())
    }

    /// Dispatch a completed HTTP response to a script's `on_http(callback,
    /// status, body)` hook.
    pub fn call_on_http(
        &self,
        path: &Path,
        callback: &str,
        status: u16,
        body: &str,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<(), String> {
        let resolved = self.resolve_path(path);
        let backend = self
            .backend_for(path)
            .ok_or_else(|| format!("No backend for {:?}", path.extension()))?;
        let commands = backend.call_on_http(&resolved, callback, status, body, ctx, vars)?;
        for cmd in commands {
            ctx.process_command(cmd);
        }
        Ok(())
    }

    /// Dispatch a player join/leave to a script's `on_player_joined(id)` /
    /// `on_player_left(id)` hook.
    pub fn call_on_player_event(
        &self,
        path: &Path,
        id: u64,
        joined: bool,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<(), String> {
        let resolved = self.resolve_path(path);
        let backend = self
            .backend_for(path)
            .ok_or_else(|| format!("No backend for {:?}", path.extension()))?;
        let commands = backend.call_on_player_event(&resolved, id, joined, ctx, vars)?;
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
        self.backends
            .first()
            .ok_or_else(|| "No backends registered".to_string())?
            .eval_expression(expr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::ScriptCommand;
    use crate::context::{ScriptTime, ScriptTransform};
    use crate::test_util::FakeBackend;

    fn ctx() -> ScriptContext {
        ScriptContext::new(ScriptTime::default(), ScriptTransform::default())
    }

    #[test]
    fn resolve_path_absolute_is_unchanged() {
        let mut engine = ScriptEngine::new();
        engine.set_scripts_folder(PathBuf::from("scripts"));
        // temp_dir is always absolute, so the scripts folder must be ignored.
        let abs = std::env::temp_dir().join("player.fake");
        assert_eq!(engine.resolve_path(&abs), abs);
    }

    #[test]
    fn resolve_path_relative_joins_scripts_folder() {
        let mut engine = ScriptEngine::new();
        engine.set_scripts_folder(PathBuf::from("proj").join("scripts"));
        assert_eq!(
            engine.resolve_path(Path::new("ai/enemy.fake")),
            PathBuf::from("proj").join("scripts").join("ai/enemy.fake")
        );
    }

    #[test]
    fn resolve_path_relative_without_folder_passes_through() {
        let engine = ScriptEngine::new();
        assert_eq!(
            engine.resolve_path(Path::new("enemy.fake")),
            PathBuf::from("enemy.fake")
        );
    }

    #[test]
    fn backend_for_dispatches_on_extension() {
        let mut engine = ScriptEngine::new();
        engine.add_backend(Box::new(FakeBackend::new("alpha", &["fake"])));
        engine.add_backend(Box::new(FakeBackend::new("beta", &["mock", "stub"])));

        let alpha = engine.backend_for(Path::new("a.fake")).unwrap();
        assert_eq!(alpha.name(), "alpha");
        // A backend with several extensions matches any of them.
        let beta = engine.backend_for(Path::new("b.stub")).unwrap();
        assert_eq!(beta.name(), "beta");
    }

    #[test]
    fn backend_for_unknown_or_missing_extension_is_none() {
        let mut engine = ScriptEngine::new();
        engine.add_backend(Box::new(FakeBackend::new("alpha", &["fake"])));
        assert!(engine.backend_for(Path::new("a.unknown")).is_none());
        assert!(engine.backend_for(Path::new("no_extension")).is_none());
    }

    #[test]
    fn add_backend_increments_backend_count() {
        let mut engine = ScriptEngine::new();
        assert_eq!(engine.backend_count(), 0);
        engine.add_backend(Box::new(FakeBackend::new("alpha", &["fake"])));
        engine.add_backend(Box::new(FakeBackend::new("beta", &["mock"])));
        assert_eq!(engine.backend_count(), 2);
    }

    #[test]
    fn set_scripts_folder_propagates_to_all_backends() {
        let a = FakeBackend::new("alpha", &["fake"]);
        let b = FakeBackend::new("beta", &["mock"]);
        let (sa, sb) = (a.state_handle(), b.state_handle());

        let mut engine = ScriptEngine::new();
        engine.add_backend(Box::new(a));
        engine.add_backend(Box::new(b));
        engine.set_scripts_folder(PathBuf::from("game/scripts"));

        assert_eq!(engine.scripts_folder(), Some(Path::new("game/scripts")));
        let folder = Some(PathBuf::from("game/scripts"));
        assert_eq!(sa.lock().unwrap().scripts_folder, folder);
        assert_eq!(sb.lock().unwrap().scripts_folder, folder);
    }

    #[test]
    fn get_available_scripts_merges_and_sorts_by_name() {
        let mut a = FakeBackend::new("alpha", &["fake"]);
        a.available = vec![
            ("zebra".to_string(), PathBuf::from("zebra.fake")),
            ("apple".to_string(), PathBuf::from("apple.fake")),
        ];
        let mut b = FakeBackend::new("beta", &["mock"]);
        b.available = vec![("mango".to_string(), PathBuf::from("mango.mock"))];

        let mut engine = ScriptEngine::new();
        engine.add_backend(Box::new(a));
        engine.add_backend(Box::new(b));

        let names: Vec<String> = engine
            .get_available_scripts()
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        assert_eq!(names, ["apple", "mango", "zebra"]);
    }

    #[test]
    fn get_available_scripts_empty_without_backends() {
        assert!(ScriptEngine::new().get_available_scripts().is_empty());
    }

    #[test]
    fn eval_expression_errors_without_backends() {
        let engine = ScriptEngine::new();
        assert_eq!(
            engine.eval_expression("1 + 1"),
            Err("No backends registered".to_string())
        );
    }

    #[test]
    fn eval_expression_uses_first_backend() {
        let mut engine = ScriptEngine::new();
        engine.add_backend(Box::new(FakeBackend::new("first", &["fake"])));
        engine.add_backend(Box::new(FakeBackend::new("second", &["mock"])));
        assert_eq!(engine.eval_expression("1 + 1"), Ok("first:1 + 1".to_string()));
    }

    #[test]
    fn call_on_ready_routes_commands_through_context() {
        let mut backend = FakeBackend::new("alpha", &["fake"]);
        backend.on_ready = || {
            Ok(vec![
                // Transform command — must land in a context field…
                ScriptCommand::SetPosition {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
                // …while non-transform commands land in `ctx.commands`.
                ScriptCommand::SpawnEntity {
                    name: "minion".to_string(),
                },
            ])
        };

        let mut engine = ScriptEngine::new();
        engine.add_backend(Box::new(backend));

        let mut ctx = ctx();
        let mut vars = crate::component::ScriptVariables::default();
        engine
            .call_on_ready(Path::new("a.fake"), &mut ctx, &mut vars)
            .unwrap();

        assert_eq!(ctx.new_position, Some(Vec3::new(1.0, 2.0, 3.0)));
        assert_eq!(ctx.commands.len(), 1);
        assert!(matches!(
            &ctx.commands[0],
            ScriptCommand::SpawnEntity { name } if name == "minion"
        ));
    }

    #[test]
    fn call_on_update_resolves_relative_path_for_backend() {
        let backend = FakeBackend::new("alpha", &["fake"]);
        let state = backend.state_handle();

        let mut engine = ScriptEngine::new();
        engine.add_backend(Box::new(backend));
        engine.set_scripts_folder(PathBuf::from("root"));

        let mut ctx = ctx();
        let mut vars = crate::component::ScriptVariables::default();
        engine
            .call_on_update(Path::new("enemy.fake"), &mut ctx, &mut vars)
            .unwrap();

        assert_eq!(
            state.lock().unwrap().update_paths,
            vec![PathBuf::from("root").join("enemy.fake")]
        );
    }

    #[test]
    fn call_on_update_without_matching_backend_errors() {
        let mut engine = ScriptEngine::new();
        engine.add_backend(Box::new(FakeBackend::new("alpha", &["fake"])));

        let mut ctx = ctx();
        let mut vars = crate::component::ScriptVariables::default();
        let err = engine
            .call_on_update(Path::new("a.unknown"), &mut ctx, &mut vars)
            .unwrap_err();
        assert!(err.contains("No backend"));
    }

    #[test]
    fn get_script_props_empty_without_matching_backend() {
        let engine = ScriptEngine::new();
        assert!(engine.get_script_props(Path::new("a.fake")).is_empty());
    }
}
