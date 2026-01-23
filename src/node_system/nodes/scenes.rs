//! Scene root nodes - the root container for all scene content

use bevy::prelude::*;

use crate::core::{EditorEntity, SceneNode};
use crate::node_system::components::NodeTypeMarker;
use crate::node_system::definition::{NodeCategory, NodeDefinition};

/// Marker component for scene root nodes
/// Only one scene root should exist per scene tab
#[derive(Component, Debug, Clone)]
pub struct SceneRoot {
    pub scene_type: SceneType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneType {
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

/// Scene3D - root node for 3D scenes
pub static SCENE3D: NodeDefinition = NodeDefinition {
    type_id: "scene.3d",
    display_name: "Scene3D",
    category: NodeCategory::Nodes3D,
    default_name: "Scene3D",
    spawn_fn: spawn_scene3d,
    serialize_fn: None,
    deserialize_fn: None,
    priority: -100, // Show first
};

/// Scene2D - root node for 2D scenes
pub static SCENE2D: NodeDefinition = NodeDefinition {
    type_id: "scene.2d",
    display_name: "Scene2D",
    category: NodeCategory::Nodes3D,
    default_name: "Scene2D",
    spawn_fn: spawn_scene2d,
    serialize_fn: None,
    deserialize_fn: None,
    priority: -99,
};

/// UI Root - root node for UI scenes
pub static UI_ROOT: NodeDefinition = NodeDefinition {
    type_id: "scene.ui",
    display_name: "UI",
    category: NodeCategory::Nodes3D,
    default_name: "UI",
    spawn_fn: spawn_ui_root,
    serialize_fn: None,
    deserialize_fn: None,
    priority: -98,
};

/// Other - generic root node
pub static OTHER_ROOT: NodeDefinition = NodeDefinition {
    type_id: "scene.other",
    display_name: "Other",
    category: NodeCategory::Nodes3D,
    default_name: "Root",
    spawn_fn: spawn_other_root,
    serialize_fn: None,
    deserialize_fn: None,
    priority: -97,
};

fn spawn_scene3d(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    _parent: Option<Entity>,
) -> Entity {
    // Scene roots are always at the root level, ignore parent
    commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: SCENE3D.default_name.to_string(),
        },
        SceneNode,
        NodeTypeMarker::new(SCENE3D.type_id),
        SceneRoot { scene_type: SceneType::Scene3D },
    )).id()
}

fn spawn_scene2d(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    _parent: Option<Entity>,
) -> Entity {
    commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: SCENE2D.default_name.to_string(),
        },
        SceneNode,
        NodeTypeMarker::new(SCENE2D.type_id),
        SceneRoot { scene_type: SceneType::Scene2D },
    )).id()
}

fn spawn_ui_root(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    _parent: Option<Entity>,
) -> Entity {
    commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: UI_ROOT.default_name.to_string(),
        },
        SceneNode,
        NodeTypeMarker::new(UI_ROOT.type_id),
        SceneRoot { scene_type: SceneType::UI },
    )).id()
}

fn spawn_other_root(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    _parent: Option<Entity>,
) -> Entity {
    commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: OTHER_ROOT.default_name.to_string(),
        },
        SceneNode,
        NodeTypeMarker::new(OTHER_ROOT.type_id),
        SceneRoot { scene_type: SceneType::Other },
    )).id()
}

/// Check if a type_id is a scene root type
pub fn is_scene_root_type(type_id: &str) -> bool {
    matches!(type_id, "scene.3d" | "scene.2d" | "scene.ui" | "scene.other")
}
