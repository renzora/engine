//! `MaterialRef` + `MaterialOverrides` components for assigning materials to entities.
//!
//! Entities with `MaterialRef` pointing to a `.material` or `.shader` file will
//! have the appropriate material resolved and applied at runtime.

use std::collections::HashMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

// Re-export from renzora_core
pub use renzora_core::MaterialRef;

/// Per-instance parameter overrides applied on top of the material's defaults.
///
/// Keys are parameter names defined in the material/shader file.
/// Values override the defaults when resolving the material.
#[derive(Component, Serialize, Deserialize, Reflect, Clone, Debug, Default)]
#[reflect(Component, Serialize, Deserialize)]
pub struct MaterialOverrides(pub HashMap<String, ParamValue>);

/// A serializable parameter value for material overrides.
#[derive(Clone, Debug, Serialize, Deserialize, Reflect)]
pub enum ParamValue {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Color([f32; 4]),
    Int(i32),
    Bool(bool),
}
