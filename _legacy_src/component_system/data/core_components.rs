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

// =============================================================================
// DISABLED COMPONENTS
// =============================================================================

/// Tracks which components are disabled (toggled off) on an entity.
/// Disabled components remain attached but their data is grayed out in the inspector.
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct DisabledComponents {
    /// Component type_ids that are currently disabled
    pub disabled: Vec<String>,
}

impl DisabledComponents {
    /// Check if a component type is disabled
    pub fn is_disabled(&self, type_id: &str) -> bool {
        self.disabled.iter().any(|id| id == type_id)
    }

    /// Toggle a component's disabled state
    pub fn toggle(&mut self, type_id: &str) {
        if let Some(pos) = self.disabled.iter().position(|id| id == type_id) {
            self.disabled.remove(pos);
        } else {
            self.disabled.push(type_id.to_string());
        }
    }
}

// =============================================================================
// WORLD ENVIRONMENT
// =============================================================================

/// Marker for world environment convenience group
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct WorldEnvironmentMarker;

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
        // Disabled components tracking
        .register_type::<DisabledComponents>()
        // Environment
        .register_type::<WorldEnvironmentMarker>()
        // Post-processing
        .register_type::<super::SkyboxData>()
        .register_type::<super::FogData>()
        .register_type::<super::AntiAliasingData>()
        .register_type::<super::AmbientOcclusionData>()
        .register_type::<super::ReflectionsData>()
        .register_type::<super::BloomData>()
        .register_type::<super::TonemappingData>()
        .register_type::<super::DepthOfFieldData>()
        .register_type::<super::MotionBlurData>();
}
