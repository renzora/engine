//! UI entity spawning (panels, labels, buttons, images)

use bevy::prelude::*;

use crate::core::{EditorEntity, SceneNode};
use crate::shared::{UIPanelData, UILabelData, UIButtonData, UIImageData};
use super::{Category, EntityTemplate};

pub static TEMPLATES: &[EntityTemplate] = &[
    EntityTemplate { name: "Panel", category: Category::UI, spawn: spawn_ui_panel },
    EntityTemplate { name: "Label", category: Category::UI, spawn: spawn_ui_label },
    EntityTemplate { name: "Button", category: Category::UI, spawn: spawn_ui_button },
    EntityTemplate { name: "Image", category: Category::UI, spawn: spawn_ui_image },
];

pub fn spawn_ui_panel(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: "Panel".to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        UIPanelData::default(),
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

pub fn spawn_ui_label(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: "Label".to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        UILabelData::default(),
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

pub fn spawn_ui_button(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: "Button".to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        UIButtonData::default(),
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

pub fn spawn_ui_image(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: "Image".to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        UIImageData::default(),
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}
