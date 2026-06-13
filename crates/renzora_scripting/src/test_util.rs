//! Test-only fake script backend shared by the engine and execution-system
//! tests. Records how the engine drove it (paths, folders, context snapshots)
//! and returns canned command lists, so tests never touch a real interpreter.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::backend::{FileReader, ScriptBackend};
use crate::command::ScriptCommand;
use crate::component::{ScriptVariableDefinition, ScriptVariables};
use crate::context::ScriptContext;

/// Everything the fake backend records about how it was driven.
#[derive(Default)]
pub struct FakeBackendState {
    pub scripts_folder: Option<PathBuf>,
    pub ready_paths: Vec<PathBuf>,
    pub update_paths: Vec<PathBuf>,
    /// `ctx.self_entity_name` captured on each `call_on_update`.
    pub seen_self_names: Vec<String>,
    /// `ctx.children` names captured on each `call_on_update`.
    pub seen_child_names: Vec<Vec<String>>,
    /// `ctx.found_entities` captured on each `call_on_update`.
    pub seen_found_entities: Vec<HashMap<String, u64>>,
}

type CommandFactory = fn() -> Result<Vec<ScriptCommand>, String>;

/// A scripted [`ScriptBackend`] double. Construct with [`FakeBackend::new`],
/// then override the public fields to shape its behavior per test.
pub struct FakeBackend {
    backend_name: &'static str,
    exts: &'static [&'static str],
    pub state: Arc<Mutex<FakeBackendState>>,
    pub available: Vec<(String, PathBuf)>,
    pub props: Vec<ScriptVariableDefinition>,
    pub on_ready: CommandFactory,
    pub on_update: CommandFactory,
}

impl FakeBackend {
    pub fn new(backend_name: &'static str, exts: &'static [&'static str]) -> Self {
        Self {
            backend_name,
            exts,
            state: Arc::new(Mutex::new(FakeBackendState::default())),
            available: Vec::new(),
            props: Vec::new(),
            on_ready: || Ok(Vec::new()),
            on_update: || Ok(Vec::new()),
        }
    }

    /// Clone the shared state handle before boxing the backend away.
    pub fn state_handle(&self) -> Arc<Mutex<FakeBackendState>> {
        self.state.clone()
    }
}

impl ScriptBackend for FakeBackend {
    fn name(&self) -> &str {
        self.backend_name
    }

    fn extensions(&self) -> &[&str] {
        self.exts
    }

    fn set_scripts_folder(&mut self, path: PathBuf) {
        self.state.lock().unwrap().scripts_folder = Some(path);
    }

    fn set_file_reader(&mut self, _reader: FileReader) {}

    fn get_available_scripts(&self) -> Vec<(String, PathBuf)> {
        self.available.clone()
    }

    fn get_script_props(&self, _path: &Path) -> Vec<ScriptVariableDefinition> {
        self.props.clone()
    }

    fn call_on_ready(
        &self,
        path: &Path,
        _ctx: &mut ScriptContext,
        _vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String> {
        self.state
            .lock()
            .unwrap()
            .ready_paths
            .push(path.to_path_buf());
        (self.on_ready)()
    }

    fn call_on_update(
        &self,
        path: &Path,
        ctx: &mut ScriptContext,
        _vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String> {
        let mut state = self.state.lock().unwrap();
        state.update_paths.push(path.to_path_buf());
        state.seen_self_names.push(ctx.self_entity_name.clone());
        state
            .seen_child_names
            .push(ctx.children.iter().map(|c| c.name.clone()).collect());
        state.seen_found_entities.push(ctx.found_entities.clone());
        (self.on_update)()
    }

    fn needs_reload(&self, _path: &Path) -> bool {
        false
    }

    fn reload(&self, _path: &Path) -> Result<(), String> {
        Ok(())
    }

    fn eval_expression(&self, expr: &str) -> Result<String, String> {
        Ok(format!("{}:{}", self.backend_name, expr))
    }
}
