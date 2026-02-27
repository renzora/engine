use bevy::prelude::*;
use serde::{Deserialize, Serialize};

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

/// Stores the user-defined display order of components in the inspector.
/// Component type_ids are stored in order. Components not in the list appear at the end.
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default)]
pub struct ComponentOrder {
    pub order: Vec<String>,
}

/// Marker component for entities visible in the editor hierarchy
#[derive(Component, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct EditorEntity {
    pub name: String,
    /// Tag for categorizing entities (e.g., "Player", "Enemy", "Pickup")
    pub tag: String,
    /// Whether the entity is visible in the viewport (eye icon in hierarchy)
    pub visible: bool,
    /// Whether the entity is locked from selection/editing (lock icon in hierarchy)
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

/// Marker for the main camera rendering to the viewport
#[derive(Component)]
pub struct MainCamera;

/// Marker for cameras that respond to viewport controls
#[derive(Component)]
pub struct ViewportCamera;

/// Marker for entities that are part of the scene (saveable)
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct SceneNode;

/// Marks which scene tab an entity belongs to
#[derive(Component, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct SceneTabId(pub usize);

/// Marker for world environment convenience group
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct WorldEnvironmentMarker;

/// Marker for audio listener node
#[derive(Component)]
pub struct AudioListenerMarker;

/// Stores the icon string for hierarchy display.
/// Set automatically when entities are spawned via presets or component-as-node.
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct NodeIcon(pub String);

/// Optional label color for visual organization in the hierarchy panel.
/// Set via right-click → Label Color. Stored as [R, G, B] bytes.
#[derive(Component, Clone, Reflect, Serialize, Deserialize, Default)]
#[reflect(Component, Default)]
pub struct EntityLabelColor(pub [u8; 3]);
