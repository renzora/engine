//! 2D entity spawning (sprites, 2D cameras)

use bevy::prelude::*;

use crate::core::{EditorEntity, SceneNode};
use crate::shared::{Sprite2DData, Camera2DData};
use super::{Category, EntityTemplate};

pub static TEMPLATES: &[EntityTemplate] = &[
    EntityTemplate { name: "Node2D", category: Category::TwoD, spawn: spawn_node2d },
    EntityTemplate { name: "Sprite2D", category: Category::TwoD, spawn: spawn_sprite2d },
    EntityTemplate { name: "Camera2D", category: Category::TwoD, spawn: spawn_camera2d },
];

pub fn spawn_node2d(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: "Node2D".to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

pub fn spawn_sprite2d(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: "Sprite2D".to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        Sprite2DData::default(),
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

pub fn spawn_camera2d(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: "Camera2D".to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        Camera2DData::default(),
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}
