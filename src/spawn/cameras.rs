//! Camera entity spawning

use bevy::prelude::*;

use crate::core::{EditorEntity, SceneNode};
use crate::shared::{CameraNodeData, CameraRigData};
use super::{Category, EntityTemplate};

pub static TEMPLATES: &[EntityTemplate] = &[
    EntityTemplate { name: "Camera3D", category: Category::Camera, spawn: spawn_camera3d },
    EntityTemplate { name: "CameraRig", category: Category::Camera, spawn: spawn_camera_rig },
];

pub fn spawn_camera3d(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        Visibility::default(),
        EditorEntity {
            name: "Camera3D".to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        CameraNodeData::default(),
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

pub fn spawn_camera_rig(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let rig_data = CameraRigData::default();
    let initial_pos = Vec3::new(0.0, rig_data.height, rig_data.distance);

    let mut entity_commands = commands.spawn((
        Transform::from_translation(initial_pos).looking_at(Vec3::ZERO, Vec3::Y),
        Visibility::default(),
        EditorEntity {
            name: "CameraRig".to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        rig_data,
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}
