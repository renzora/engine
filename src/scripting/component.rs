//! Script component that can be attached to entities

#![allow(dead_code)]

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Component that marks an entity as having a script attached
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct ScriptComponent {
    /// The script identifier (registered name) - for built-in scripts
    pub script_id: String,
    /// Path to .rhai script file (relative to project) - for file-based scripts
    pub script_path: Option<std::path::PathBuf>,
    /// Whether the script is enabled
    pub enabled: bool,
    /// Script-local variables (persisted)
    pub variables: ScriptVariables,
    /// Runtime state (not persisted)
    #[allow(dead_code)]
    #[reflect(ignore)]
    pub runtime_state: ScriptRuntimeState,
}

impl ScriptComponent {
    pub fn new(script_id: impl Into<String>) -> Self {
        Self {
            script_id: script_id.into(),
            script_path: None,
            enabled: true,
            variables: ScriptVariables::default(),
            runtime_state: ScriptRuntimeState::default(),
        }
    }

    pub fn from_file(path: std::path::PathBuf) -> Self {
        Self {
            script_id: String::new(),
            script_path: Some(path),
            enabled: true,
            variables: ScriptVariables::default(),
            runtime_state: ScriptRuntimeState::default(),
        }
    }

    pub fn with_variable(mut self, name: impl Into<String>, value: ScriptValue) -> Self {
        self.variables.set(name, value);
        self
    }

    /// Check if this is a file-based script
    pub fn is_file_script(&self) -> bool {
        self.script_path.is_some()
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
