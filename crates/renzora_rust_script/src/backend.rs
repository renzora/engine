//! A [`ScriptBackend`] that claims `.rs`, so Rust scripts attach the same way
//! Lua ones do.
//!
//! # Why it does nothing
//!
//! The scripting layer's execution model is that a backend returns
//! [`ScriptCommand`]s and a queue applies them. That is deliberate — it keeps
//! backends safe and interchangeable — and it is exactly what a Rust script does
//! not want: the whole reason to write one is `&mut World`, which no command
//! vocabulary can stand in for.
//!
//! So the execution happens in [`crate::dispatch`] instead, in an exclusive
//! system that hands the script the real world. This backend exists for the
//! other half of what a backend does: **claiming the extension**.
//!
//! Without it, `ScriptEngine::backend_for` returns `None` for a `.rs` entry and
//! the execution loop reports `No backend for Some("rs")`, latching
//! `runtime_state.has_error` so the inspector shows the script as broken. It is
//! not broken; it is simply not run from there.
//!
//! The alternative was a separate `RustScript` component, which worked but meant
//! two ways to attach a script and a Scripts panel that quietly could not accept
//! half of them.

use std::path::{Path, PathBuf};

// Everything here is glob-re-exported at the crate root (`pub use backend::*`).
use renzora_scripting::{
    FileReader, ScriptBackend, ScriptCommand, ScriptContext, ScriptVariableDefinition,
    ScriptVariables,
};

#[derive(Default)]
pub struct RustScriptBackend {
    scripts_folder: PathBuf,
}

impl ScriptBackend for RustScriptBackend {
    fn name(&self) -> &str {
        "Rust"
    }

    fn extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn set_scripts_folder(&mut self, path: PathBuf) {
        self.scripts_folder = path;
    }

    fn set_file_reader(&mut self, _reader: FileReader) {
        // Nothing to read: a Rust script is compiled to a library before it runs,
        // so its source is never loaded at execution time.
    }

    fn get_available_scripts(&self) -> Vec<(String, PathBuf)> {
        let Ok(entries) = std::fs::read_dir(&self.scripts_folder) else {
            return Vec::new();
        };
        let mut out: Vec<(String, PathBuf)> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rs"))
            .filter_map(|p| {
                let name = p.file_name()?.to_str()?.to_string();
                Some((name, p))
            })
            .collect();
        // Stable order, so the picker does not reshuffle between launches.
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    fn get_script_props(&self, _path: &Path) -> Vec<ScriptVariableDefinition> {
        // Lua declares props in a table the backend parses. The Rust equivalent
        // would be reading attributes off the source, which is real work and not
        // done yet — a Rust script's tunables are components on the entity for
        // now, which the inspector already edits.
        Vec::new()
    }

    fn call_on_ready(
        &self,
        _path: &Path,
        _ctx: &mut ScriptContext,
        _vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String> {
        Ok(Vec::new())
    }

    fn call_on_update(
        &self,
        _path: &Path,
        _ctx: &mut ScriptContext,
        _vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String> {
        // See the module doc: the real call is in `crate::dispatch`, which has
        // `&mut World`. Returning no commands here is not a stub — there are
        // genuinely none to return.
        Ok(Vec::new())
    }

    /// Always false. Reloading a Rust script means recompiling and mapping a new
    /// library, which this backend does not own — [`crate::compile_and_load`]
    /// does, and only when the project opens. Answering `true` would invite the
    /// engine to call [`reload`](Self::reload), which cannot do anything useful.
    fn needs_reload(&self, _path: &Path) -> bool {
        false
    }

    fn reload(&self, _path: &Path) -> Result<(), String> {
        Err("rust scripts reload by recompiling; restart the editor".to_string())
    }

    /// No REPL. A Lua backend can evaluate a string because it carries an
    /// interpreter; evaluating Rust would mean invoking the compiler per
    /// expression and mapping a library for each result.
    fn eval_expression(&self, _expr: &str) -> Result<String, String> {
        Err("rust scripts cannot evaluate expressions".to_string())
    }
}
