use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single script entry within a ScriptComponent
#[derive(Clone, Reflect)]
pub struct ScriptEntry {
    pub id: u32,
    /// For built-in/registered scripts
    pub script_id: String,
    /// Path to script file (relative to project)
    pub script_path: Option<std::path::PathBuf>,
    pub enabled: bool,
    pub variables: ScriptVariables,
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

    pub fn is_file_script(&self) -> bool {
        self.script_path.is_some()
    }
}

/// Component that marks an entity as having scripts attached
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct ScriptComponent {
    pub scripts: Vec<ScriptEntry>,
    next_id: u32,
}

impl ScriptComponent {
    pub fn new() -> Self {
        Self { scripts: Vec::new(), next_id: 1 }
    }

    pub fn with_script(script_id: impl Into<String>) -> Self {
        let mut comp = Self::new();
        comp.add_script(script_id);
        comp
    }

    pub fn from_file(path: std::path::PathBuf) -> Self {
        let mut comp = Self::new();
        comp.add_file_script(path);
        comp
    }

    pub fn add_script(&mut self, script_id: impl Into<String>) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.scripts.push(ScriptEntry::new(id, script_id));
        id
    }

    pub fn add_file_script(&mut self, path: std::path::PathBuf) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.scripts.push(ScriptEntry::from_file(id, path));
        id
    }

    pub fn remove_script(&mut self, index: usize) {
        if index < self.scripts.len() {
            self.scripts.remove(index);
        }
    }

    pub fn with_variable(mut self, name: impl Into<String>, value: ScriptValue) -> Self {
        if let Some(entry) = self.scripts.last_mut() {
            entry.variables.set(name, value);
        }
        self
    }
}

/// Script-local variables that can be set from the inspector
#[derive(Clone, Default, Reflect, Serialize, Deserialize)]
pub struct ScriptVariables {
    values: HashMap<String, ScriptValue>,
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

    pub fn iter_all(&self) -> impl Iterator<Item = (&String, &ScriptValue)> {
        self.values.iter()
    }
}

/// Value types for script variables
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub enum ScriptValue {
    Float(f32),
    Int(i32),
    Bool(bool),
    String(String),
    Entity(String),
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
            ScriptValue::Entity(_) => "Entity",
            ScriptValue::Vec2(_) => "Vec2",
            ScriptValue::Vec3(_) => "Vec3",
            ScriptValue::Color(_) => "Color",
        }
    }
}

/// Definition of an inspector-exposed variable
#[derive(Clone)]
pub struct ScriptVariableDefinition {
    pub name: String,
    pub display_name: String,
    pub default_value: ScriptValue,
    pub hint: Option<String>,
    /// Optional inspector group. Props with the same tab render under one
    /// collapsible header; `None` falls into the default "General" group.
    pub tab: Option<String>,
}

impl ScriptVariableDefinition {
    pub fn new(name: impl Into<String>, default: ScriptValue) -> Self {
        let name = name.into();
        Self {
            display_name: name.clone(),
            name,
            default_value: default,
            hint: None,
            tab: None,
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

    pub fn with_tab(mut self, tab: impl Into<String>) -> Self {
        self.tab = Some(tab.into());
        self
    }
}

/// Runtime state (not persisted)
#[derive(Clone, Default)]
pub struct ScriptRuntimeState {
    pub initialized: bool,
    pub last_frame: u64,
    pub has_error: bool,
    pub last_script_modified: Option<std::time::SystemTime>,
}
