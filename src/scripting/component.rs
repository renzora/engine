//! Script component that can be attached to entities

#![allow(dead_code)]

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// A single script entry within a ScriptComponent
#[derive(Clone, Reflect)]
pub struct ScriptEntry {
    /// Unique ID within this component
    pub id: u32,
    /// The script identifier (registered name) - for built-in scripts
    pub script_id: String,
    /// Path to .rhai script file (relative to project) - for file-based scripts
    pub script_path: Option<std::path::PathBuf>,
    /// Whether the script is enabled
    pub enabled: bool,
    /// Script-local variables (persisted)
    pub variables: ScriptVariables,
    /// Runtime state (not persisted)
    #[reflect(ignore)]
    pub runtime_state: ScriptRuntimeState,
}

impl ScriptEntry {
    pub fn new(id: u32, script_id: impl Into<String>) -> Self {
        Self {
            id,
            script_id: script_id.into(),
            script_path: None,
            enabled: true,
            variables: ScriptVariables::default(),
            runtime_state: ScriptRuntimeState::default(),
        }
    }

    pub fn from_file(id: u32, path: std::path::PathBuf) -> Self {
        Self {
            id,
            script_id: String::new(),
            script_path: Some(path),
            enabled: true,
            variables: ScriptVariables::default(),
            runtime_state: ScriptRuntimeState::default(),
        }
    }

    /// Check if this is a file-based script
    pub fn is_file_script(&self) -> bool {
        self.script_path.is_some()
    }
}

/// Component that marks an entity as having scripts attached
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct ScriptComponent {
    /// List of script entries
    pub scripts: Vec<ScriptEntry>,
    /// Next ID for auto-incrementing
    next_id: u32,
}

impl ScriptComponent {
    /// Create a new empty ScriptComponent
    pub fn new() -> Self {
        Self {
            scripts: Vec::new(),
            next_id: 1,
        }
    }

    /// Create with a single named script
    pub fn with_script(script_id: impl Into<String>) -> Self {
        let mut comp = Self::new();
        comp.add_script(script_id);
        comp
    }

    /// Create with a single file-based script
    pub fn from_file(path: std::path::PathBuf) -> Self {
        let mut comp = Self::new();
        comp.add_file_script(path);
        comp
    }

    /// Add a named script entry, returns the entry ID
    pub fn add_script(&mut self, script_id: impl Into<String>) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.scripts.push(ScriptEntry::new(id, script_id));
        id
    }

    /// Add a file-based script entry, returns the entry ID
    pub fn add_file_script(&mut self, path: std::path::PathBuf) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.scripts.push(ScriptEntry::from_file(id, path));
        id
    }

    /// Remove a script entry by index
    pub fn remove_script(&mut self, index: usize) {
        if index < self.scripts.len() {
            self.scripts.remove(index);
        }
    }

    /// Add a variable to a specific script entry
    pub fn with_variable(mut self, name: impl Into<String>, value: ScriptValue) -> Self {
        if let Some(entry) = self.scripts.last_mut() {
            entry.variables.set(name, value);
        }
        self
    }

    // === Legacy compatibility accessors ===

    /// Get the script_id of the first entry (for backward compat)
    pub fn script_id(&self) -> &str {
        self.scripts.first().map(|e| e.script_id.as_str()).unwrap_or("")
    }

    /// Get the script_path of the first entry (for backward compat)
    pub fn script_path(&self) -> Option<&std::path::PathBuf> {
        self.scripts.first().and_then(|e| e.script_path.as_ref())
    }

    /// Get the enabled state of the first entry (for backward compat)
    pub fn enabled(&self) -> bool {
        self.scripts.first().map(|e| e.enabled).unwrap_or(true)
    }

    /// Get the runtime_state of the first entry (for backward compat)
    pub fn runtime_state(&self) -> &ScriptRuntimeState {
        static DEFAULT: ScriptRuntimeState = ScriptRuntimeState {
            initialized: false,
            last_frame: 0,
            has_error: false,
        };
        self.scripts.first().map(|e| &e.runtime_state).unwrap_or(&DEFAULT)
    }

    /// Check if this has a file-based script (any entry)
    pub fn is_file_script(&self) -> bool {
        self.scripts.iter().any(|e| e.is_file_script())
    }

    /// Migrate from legacy single-script format
    pub fn from_legacy(
        script_id: String,
        script_path: Option<std::path::PathBuf>,
        enabled: bool,
        variables: ScriptVariables,
    ) -> Self {
        let mut comp = Self::new();
        let id = comp.next_id;
        comp.next_id += 1;
        comp.scripts.push(ScriptEntry {
            id,
            script_id,
            script_path,
            enabled,
            variables,
            runtime_state: ScriptRuntimeState::default(),
        });
        comp
    }
}

/// Script-local variables that can be set from the inspector
#[derive(Clone, Default, Reflect, Serialize, Deserialize)]
pub struct ScriptVariables {
    values: std::collections::HashMap<String, ScriptValue>,
}

impl ScriptVariables {
    pub fn get(&self, name: &str) -> Option<&ScriptValue> {
        self.values.get(name)
    }

    pub fn get_float(&self, name: &str) -> Option<f32> {
        match self.values.get(name)? {
            ScriptValue::Float(v) => Some(*v),
            _ => None,
        }
    }

    pub fn get_int(&self, name: &str) -> Option<i32> {
        match self.values.get(name)? {
            ScriptValue::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub fn get_bool(&self, name: &str) -> Option<bool> {
        match self.values.get(name)? {
            ScriptValue::Bool(v) => Some(*v),
            _ => None,
        }
    }

    pub fn get_string(&self, name: &str) -> Option<&str> {
        match self.values.get(name)? {
            ScriptValue::String(v) => Some(v),
            _ => None,
        }
    }

    pub fn get_vec3(&self, name: &str) -> Option<Vec3> {
        match self.values.get(name)? {
            ScriptValue::Vec3(v) => Some(*v),
            _ => None,
        }
    }

    pub fn set(&mut self, name: impl Into<String>, value: ScriptValue) {
        self.values.insert(name.into(), value);
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut ScriptValue> {
        self.values.get_mut(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &ScriptValue)> {
        self.values.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&String, &mut ScriptValue)> {
        self.values.iter_mut()
    }

    pub fn iter_all(&self) -> impl Iterator<Item = (&String, &ScriptValue)> {
        self.values.iter()
    }
}

/// Value types that can be stored in script variables
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub enum ScriptValue {
    Float(f32),
    Int(i32),
    Bool(bool),
    String(String),
    Vec2(Vec2),
    Vec3(Vec3),
    Color(Vec4),
}

impl ScriptValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            ScriptValue::Float(_) => "Float",
            ScriptValue::Int(_) => "Int",
            ScriptValue::Bool(_) => "Bool",
            ScriptValue::String(_) => "String",
            ScriptValue::Vec2(_) => "Vec2",
            ScriptValue::Vec3(_) => "Vec3",
            ScriptValue::Color(_) => "Color",
        }
    }
}

/// Runtime state for a script (not saved)
#[derive(Clone, Default)]
pub struct ScriptRuntimeState {
    /// Whether on_ready has been called
    pub initialized: bool,
    /// Frame count when script was last run
    pub last_frame: u64,
    /// Whether the script has a load/compile error (to avoid log spam)
    pub has_error: bool,
}

/// Defines a variable that can be exposed in the inspector
#[derive(Clone)]
pub struct ScriptVariableDefinition {
    pub name: String,
    pub display_name: String,
    pub default_value: ScriptValue,
    pub hint: Option<String>,
}

impl ScriptVariableDefinition {
    pub fn new(name: impl Into<String>, default: ScriptValue) -> Self {
        let name = name.into();
        Self {
            display_name: name.clone(),
            name,
            default_value: default,
            hint: None,
        }
    }

    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = name.into();
        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}
