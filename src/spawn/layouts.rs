//! Pre-built scene layouts for testing and prototyping

use bevy::prelude::*;
use std::f32::consts::FRAC_PI_4;

use crate::core::{EditorEntity, SceneNode};
use crate::shared::{
    DirectionalLightData, MeshNodeData, MeshPrimitiveType,
    PhysicsBodyData, PhysicsBodyType, CollisionShapeData, CollisionShapeType,
};
use super::{Category, EntityTemplate, EditorSceneRoot, SceneType};

pub static TEMPLATES: &[EntityTemplate] = &[
    EntityTemplate { name: "Physics Test", category: Category::Layout, spawn: spawn_physics_test },
];

/// Spawn a complete physics test scene with ground, ramp, and dynamic objects.
pub fn spawn_physics_test(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    _parent: Option<Entity>,
) -> Entity {
    // --- Scene root ---
    let root = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: "PhysicsTest".to_string(),
            tag: String::new(),
            visible: true,
            locked: false,
        },
        SceneNode,
        EditorSceneRoot { scene_type: SceneType::Scene3D },
    )).id();

    // --- Ground plane ---
    let ground_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.55, 0.42),
        perceptual_roughness: 0.95,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(20.0, 20.0))),
        MeshMaterial3d(ground_mat),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Visibility::default(),
        EditorEntity { name: "Ground".into(), tag: String::new(), visible: true, locked: false },
        SceneNode,
        MeshNodeData { mesh_type: MeshPrimitiveType::Plane },
        PhysicsBodyData::static_body(),
        CollisionShapeData {
            shape_type: CollisionShapeType::Box,
            half_extents: Vec3::new(10.0, 0.05, 10.0),
            friction: 0.8,
            ..default()
        },
        ChildOf(root),
    ));

    // --- Ramp ---
    let ramp_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.6, 0.55, 0.5),
        perceptual_roughness: 0.85,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(4.0, 0.2, 3.0))),
        MeshMaterial3d(ramp_mat),
        Transform::from_xyz(-3.0, 1.0, 0.0)
            .with_rotation(Quat::from_rotation_z(FRAC_PI_4 * 0.5)),
        Visibility::default(),
        EditorEntity { name: "Ramp".into(), tag: String::new(), visible: true, locked: false },
        SceneNode,
        MeshNodeData { mesh_type: MeshPrimitiveType::Cube },
        PhysicsBodyData::static_body(),
        CollisionShapeData {
            shape_type: CollisionShapeType::Box,
            half_extents: Vec3::new(2.0, 0.1, 1.5),
            friction: 0.6,
            ..default()
        },
        ChildOf(root),
    ));

    // --- Platform / shelf ---
    let platform_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.55, 0.6),
        perceptual_roughness: 0.85,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(3.0, 0.3, 3.0))),
        MeshMaterial3d(platform_mat),
        Transform::from_xyz(4.0, 2.0, 0.0),
        Visibility::default(),
        EditorEntity { name: "Platform".into(), tag: String::new(), visible: true, locked: false },
        SceneNode,
        MeshNodeData { mesh_type: MeshPrimitiveType::Cube },
        PhysicsBodyData::static_body(),
        CollisionShapeData {
            shape_type: CollisionShapeType::Box,
            half_extents: Vec3::new(1.5, 0.15, 1.5),
            friction: 0.7,
            ..default()
        },
        ChildOf(root),
    ));

    // --- Dynamic cubes ---
    let cube_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.35, 0.25),
        perceptual_roughness: 0.7,
        ..default()
    });
    // Cube 1: dropped from height
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(cube_mat.clone()),
        Transform::from_xyz(0.0, 5.0, 0.0),
        Visibility::default(),
        EditorEntity { name: "DynamicCube".into(), tag: String::new(), visible: true, locked: false },
        SceneNode,
        MeshNodeData { mesh_type: MeshPrimitiveType::Cube },
        PhysicsBodyData { mass: 1.0, ..default() },
        CollisionShapeData {
            half_extents: Vec3::splat(0.5),
            friction: 0.5,
            restitution: 0.2,
            ..default()
        },
        ChildOf(root),
    ));

    // Cube 2: heavy, on the platform
    let heavy_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.7, 0.25, 0.25),
        perceptual_roughness: 0.6,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(heavy_mat),
        Transform::from_xyz(4.0, 4.0, 0.0),
        Visibility::default(),
        EditorEntity { name: "HeavyCube".into(), tag: String::new(), visible: true, locked: false },
        SceneNode,
        MeshNodeData { mesh_type: MeshPrimitiveType::Cube },
        PhysicsBodyData { mass: 5.0, ..default() },
        CollisionShapeData {
            half_extents: Vec3::splat(0.5),
            friction: 0.6,
            restitution: 0.1,
            ..default()
        },
        ChildOf(root),
    ));

    // Cube 3: on the ramp
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.6, 0.6, 0.6))),
        MeshMaterial3d(cube_mat.clone()),
        Transform::from_xyz(-2.5, 3.0, 0.0),
        Visibility::default(),
        EditorEntity { name: "RampCube".into(), tag: String::new(), visible: true, locked: false },
        SceneNode,
        MeshNodeData { mesh_type: MeshPrimitiveType::Cube },
        PhysicsBodyData { mass: 0.5, ..default() },
        CollisionShapeData {
            half_extents: Vec3::splat(0.3),
            friction: 0.4,
            restitution: 0.15,
            ..default()
        },
        ChildOf(root),
    ));

    // --- Bouncy sphere ---
    let sphere_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.6, 0.85),
        perceptual_roughness: 0.3,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.5).mesh().ico(5).unwrap())),
        MeshMaterial3d(sphere_mat),
        Transform::from_xyz(1.5, 6.0, 1.5),
        Visibility::default(),
        EditorEntity { name: "BouncySphere".into(), tag: String::new(), visible: true, locked: false },
        SceneNode,
        MeshNodeData { mesh_type: MeshPrimitiveType::Sphere },
        PhysicsBodyData { mass: 1.0, ..default() },
        CollisionShapeData {
            shape_type: CollisionShapeType::Sphere,
            radius: 0.5,
            friction: 0.3,
            restitution: 0.8,
            ..default()
        },
        ChildOf(root),
    ));

    // --- Cylinder ---
    let cyl_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.7, 0.3),
        perceptual_roughness: 0.5,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.4, 1.2))),
        MeshMaterial3d(cyl_mat),
        Transform::from_xyz(-1.0, 4.0, -2.0),
        Visibility::default(),
        EditorEntity { name: "DynamicCylinder".into(), tag: String::new(), visible: true, locked: false },
        SceneNode,
        MeshNodeData { mesh_type: MeshPrimitiveType::Cylinder },
        PhysicsBodyData { mass: 1.5, ..default() },
        CollisionShapeData {
            shape_type: CollisionShapeType::Cylinder,
            radius: 0.4,
            half_height: 0.6,
            friction: 0.5,
            restitution: 0.3,
            ..default()
        },
        ChildOf(root),
    ));

    // --- Capsule ---
    let capsule_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.6, 0.35, 0.7),
        perceptual_roughness: 0.4,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Capsule3d::new(0.3, 0.8).mesh().build())),
        MeshMaterial3d(capsule_mat),
        Transform::from_xyz(2.0, 5.0, -1.0),
        Visibility::default(),
        EditorEntity { name: "DynamicCapsule".into(), tag: String::new(), visible: true, locked: false },
        SceneNode,
        MeshNodeData { mesh_type: MeshPrimitiveType::Cylinder },
        PhysicsBodyData { mass: 1.0, ..default() },
        CollisionShapeData {
            shape_type: CollisionShapeType::Capsule,
            radius: 0.3,
            half_height: 0.4,
            friction: 0.4,
            restitution: 0.5,
            ..default()
        },
        ChildOf(root),
    ));

    // --- Stack of small cubes (domino test) ---
    let stack_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.6, 0.2),
        perceptual_roughness: 0.6,
        ..default()
    });
    for i in 0..4 {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.5, 0.5, 0.5))),
            MeshMaterial3d(stack_mat.clone()),
            Transform::from_xyz(0.0, 0.25 + i as f32 * 0.55, -4.0),
            Visibility::default(),
            EditorEntity {
                name: format!("StackCube{}", i + 1),
                tag: String::new(),
                visible: true,
                locked: false,
            },
            SceneNode,
            MeshNodeData { mesh_type: MeshPrimitiveType::Cube },
            PhysicsBodyData { mass: 0.8, ..default() },
            CollisionShapeData {
                half_extents: Vec3::splat(0.25),
                friction: 0.7,
                restitution: 0.05,
                ..default()
            },
            ChildOf(root),
        ));
    }

    // --- Directional light ---
    let light_data = DirectionalLightData {
        shadows_enabled: true,
        illuminance: 12000.0,
        ..default()
    };
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(light_data.color.x, light_data.color.y, light_data.color.z),
            illuminance: light_data.illuminance,
            shadows_enabled: light_data.shadows_enabled,
            ..default()
        },
        light_data,
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, 0.4, 0.0)),
        Visibility::default(),
        EditorEntity { name: "Sun".into(), tag: String::new(), visible: true, locked: false },
        SceneNode,
        ChildOf(root),
    ));

    root
}
