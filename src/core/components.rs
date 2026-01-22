use bevy::prelude::*;

/// Marker component for entities visible in the editor hierarchy
#[derive(Component)]
pub struct EditorEntity {
    pub name: String,
}

/// Marker for the main camera rendering to the viewport
#[derive(Component)]
pub struct MainCamera;

/// Marker for cameras that respond to viewport controls
#[derive(Component)]
pub struct ViewportCamera;

/// Marker for entities that are part of the scene (saveable)
#[derive(Component)]
pub struct SceneNode;

/// Marks which scene tab an entity belongs to
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub struct SceneTabId(pub usize);

/// Marker for world environment node (ambient light, fog, clear color)
#[derive(Component)]
pub struct WorldEnvironmentMarker {
    pub data: crate::scene_file::WorldEnvironmentData,
}

/// Marker for audio listener node
#[derive(Component)]
pub struct AudioListenerMarker;
