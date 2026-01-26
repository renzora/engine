//! BlueprintComponent for attaching blueprints to entities

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::{BlueprintGraph, PinValue};

/// Component that attaches a blueprint to an entity
#[derive(Component, Clone, Debug, Serialize, Deserialize, Reflect)]
#[reflect(Component)]
pub struct BlueprintComponent {
    /// Path to the blueprint file
    pub blueprint_path: String,

    /// Whether the blueprint is enabled
    pub enabled: bool,

    /// Variable overrides (variable name -> value) - serialized as JSON string
    #[serde(default)]
    #[reflect(ignore)]
    pub variable_overrides: HashMap<String, BlueprintValue>,

    /// Runtime state (not serialized)
    #[serde(skip)]
    #[reflect(ignore)]
    pub runtime_state: BlueprintRuntimeState,
}

/// Simplified value type for serialization (mirrors PinValue)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BlueprintValue {
    Float(f32),
    Int(i32),
    Bool(bool),
    String(String),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Color([f32; 4]),
}

impl From<PinValue> for BlueprintValue {
    fn from(value: PinValue) -> Self {
        match value {
            PinValue::Flow => BlueprintValue::Bool(false),
            PinValue::Float(v) => BlueprintValue::Float(v),
            PinValue::Int(v) => BlueprintValue::Int(v),
            PinValue::Bool(v) => BlueprintValue::Bool(v),
            PinValue::String(v) => BlueprintValue::String(v),
            PinValue::Vec2(v) => BlueprintValue::Vec2(v),
            PinValue::Vec3(v) => BlueprintValue::Vec3(v),
            PinValue::Color(v) => BlueprintValue::Color(v),
        }
    }
}

impl From<BlueprintValue> for PinValue {
    fn from(value: BlueprintValue) -> Self {
        match value {
            BlueprintValue::Float(v) => PinValue::Float(v),
            BlueprintValue::Int(v) => PinValue::Int(v),
            BlueprintValue::Bool(v) => PinValue::Bool(v),
            BlueprintValue::String(v) => PinValue::String(v),
            BlueprintValue::Vec2(v) => PinValue::Vec2(v),
            BlueprintValue::Vec3(v) => PinValue::Vec3(v),
            BlueprintValue::Color(v) => PinValue::Color(v),
        }
    }
}

/// Runtime state for a blueprint instance
#[derive(Clone, Debug, Default)]
pub struct BlueprintRuntimeState {
    /// Whether on_ready has been called
    pub initialized: bool,

    /// Cached compiled Rhai code
    pub compiled_code: Option<String>,

    /// Runtime variable values
    pub variables: HashMap<String, PinValue>,
}

impl Default for BlueprintComponent {
    fn default() -> Self {
        Self {
            blueprint_path: String::new(),
            enabled: true,
            variable_overrides: HashMap::new(),
            runtime_state: BlueprintRuntimeState::default(),
        }
    }
}

impl BlueprintComponent {
    /// Create a new blueprint component with the given path
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            blueprint_path: path.into(),
            enabled: true,
            variable_overrides: HashMap::new(),
            runtime_state: BlueprintRuntimeState::default(),
        }
    }

    /// Get the name from the path
    pub fn name(&self) -> &str {
        self.blueprint_path
            .rsplit('/')
            .next()
            .unwrap_or(&self.blueprint_path)
            .trim_end_matches(".blueprint")
    }

    /// Mark as needing recompilation
    pub fn invalidate(&mut self) {
        self.runtime_state.compiled_code = None;
    }
}
