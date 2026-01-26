//! Scene root entity spawning
//!
//! Scene roots are the top-level containers for different types of scene content.

use bevy::prelude::*;

use crate::core::{EditorEntity, SceneNode};
use super::EntityTemplate;

/// Marker component for scene root nodes in the editor hierarchy.
/// Named EditorSceneRoot to avoid collision with Bevy's SceneRoot.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct EditorSceneRoot {
    pub scene_type: SceneType,
}

/// Type of scene root
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum SceneType {
    #[default]
    Scene3D,
    Scene2D,
    UI,
    Other,
}

impl SceneType {
    pub fn display_name(&self) -> &'static str {
        match self {
            SceneType::Scene3D => "3D Scene",
            SceneType::Scene2D => "2D Scene",
            SceneType::UI => "UI",
            SceneType::Other => "Other",
        }
    }

    pub fn default_node_name(&self) -> &'static str {
        match self {
            SceneType::Scene3D => "Scene3D",
            SceneType::Scene2D => "Scene2D",
            SceneType::UI => "UI",
            SceneType::Other => "Root",
        }
    }
}

// Note: Scene roots are special and not in the regular add menu
// They are spawned separately via the "Create Root" UI
pub static TEMPLATES: &[EntityTemplate] = &[];

pub fn spawn_scene3d(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    _parent: Option<Entity>,
) -> Entity {
    commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: "Scene3D".to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        EditorSceneRoot { scene_type: SceneType::Scene3D },
    )).id()
}

pub fn spawn_scene2d(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    _parent: Option<Entity>,
) -> Entity {
    commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: "Scene2D".to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        EditorSceneRoot { scene_type: SceneType::Scene2D },
    )).id()
}

pub fn spawn_ui_root(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    _parent: Option<Entity>,
) -> Entity {
    commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: "UI".to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        EditorSceneRoot { scene_type: SceneType::UI },
    )).id()
}

pub fn spawn_other_root(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    _parent: Option<Entity>,
) -> Entity {
    commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: "Root".to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        EditorSceneRoot { scene_type: SceneType::Other },
    )).id()
}

/// Check if a component indicates a scene root type
pub fn is_scene_root(_scene_root: &EditorSceneRoot) -> bool {
    true // All EditorSceneRoot components are scene roots
}
