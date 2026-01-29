//! Core components shared between editor and runtime
//!
//! These are fundamental components that both editor and runtime need
//! for entity identification, scene organization, and gameplay.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

// =============================================================================
// ENTITY IDENTIFICATION
// =============================================================================

/// Marker component for entities visible in the editor hierarchy.
/// Also used at runtime for entity identification by name and tag.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct EditorEntity {
    /// Display name of the entity
    pub name: String,
    /// Tag for categorizing entities (e.g., "Player", "Enemy", "Pickup")
    /// Multiple tags can be comma-separated
    pub tag: String,
    /// Whether the entity is visible in the viewport
    pub visible: bool,
    /// Whether the entity is locked from selection/editing (editor only)
    pub locked: bool,
}

impl Default for EditorEntity {
    fn default() -> Self {
        Self {
            name: String::new(),
            tag: String::new(),
            visible: true,
            locked: false,
        }
    }
}

// =============================================================================
// SCENE ORGANIZATION
// =============================================================================

/// Marker for entities that are part of the scene (saveable)
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct SceneNode;

/// Marks which scene tab an entity belongs to (editor only)
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct SceneTabId(pub usize);

// =============================================================================
// GAMEPLAY COMPONENTS
// =============================================================================

/// Health component for entities that can take damage
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct HealthData {
    /// Maximum health value
    pub max_health: f32,
    /// Current health value
    pub current_health: f32,
    /// Health regeneration rate per second
    pub regeneration_rate: f32,
    /// Whether the entity is currently invincible
    pub invincible: bool,
    /// Whether to destroy the entity when health reaches 0
    pub destroy_on_death: bool,
}

impl Default for HealthData {
    fn default() -> Self {
        Self {
            max_health: 100.0,
            current_health: 100.0,
            regeneration_rate: 0.0,
            invincible: false,
            destroy_on_death: true,
        }
    }
}

// =============================================================================
// SCRIPTING
// =============================================================================

/// Runtime state for a script (not serialized)
#[derive(Clone, Debug, Default)]
pub struct ScriptRuntimeState {
    /// Whether the script has been initialized (on_ready called)
    pub initialized: bool,
    /// Whether the script has encountered an error
    pub has_error: bool,
    /// Last error message if any
    pub last_error: Option<String>,
}

/// Component for attaching Rhai scripts to entities
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct ScriptComponent {
    /// Path to the script file (relative to project's scripts folder)
    pub script_path: Option<String>,
    /// Whether the script is enabled
    pub enabled: bool,
    /// Script variables (exposed to editor, serialized)
    #[serde(default)]
    pub variables: std::collections::HashMap<String, ScriptVariableValue>,
    /// Runtime state (not serialized)
    #[serde(skip)]
    #[reflect(ignore)]
    pub runtime_state: ScriptRuntimeState,
}

impl Default for ScriptComponent {
    fn default() -> Self {
        Self {
            script_path: None,
            enabled: true,
            variables: std::collections::HashMap::new(),
            runtime_state: ScriptRuntimeState::default(),
        }
    }
}

/// Value types for script variables
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub enum ScriptVariableValue {
    Float(f64),
    Int(i64),
    Bool(bool),
    String(String),
    Vec2([f64; 2]),
    Vec3([f64; 3]),
    Color([f64; 4]),
}

impl Default for ScriptVariableValue {
    fn default() -> Self {
        Self::Float(0.0)
    }
}

// =============================================================================
// WORLD ENVIRONMENT
// =============================================================================

/// Marker for world environment node
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct WorldEnvironmentMarker {
    /// Environment data
    pub data: super::WorldEnvironmentData,
}

impl Default for WorldEnvironmentMarker {
    fn default() -> Self {
        Self {
            data: super::WorldEnvironmentData::default(),
        }
    }
}

// =============================================================================
// REGISTRATION
// =============================================================================

/// Register all core component types for reflection
pub fn register_core_types(app: &mut App) {
    app
        // Entity identification
        .register_type::<EditorEntity>()
        .register_type::<SceneNode>()
        .register_type::<SceneTabId>()
        // Gameplay
        .register_type::<HealthData>()
        // Scripting
        .register_type::<ScriptComponent>()
        .register_type::<ScriptVariableValue>()
        // Environment
        .register_type::<WorldEnvironmentMarker>();
}
