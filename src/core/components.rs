use bevy::prelude::*;

/// Marker component for entities visible in the editor hierarchy
#[derive(Component, Reflect)]
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

/// Marker for world environment node (ambient light, fog, clear color)
#[derive(Component)]
pub struct WorldEnvironmentMarker {
    pub data: crate::shared::WorldEnvironmentData,
}

/// Marker for audio listener node
#[derive(Component)]
pub struct AudioListenerMarker;
