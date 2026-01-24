//! Shared node spawning logic for editor and runtime
//!
//! This module contains the core spawning functions that create Bevy entities
//! from scene node data. Both the editor and runtime use this to ensure
//! consistent behavior.

use bevy::prelude::*;
use std::collections::HashMap;

use super::components::{
    CameraNodeData, CollisionShapeData, MeshInstanceData, MeshNodeData, MeshPrimitiveType,
    PhysicsBodyData, SceneInstanceData,
};
use super::scene_format::NodeData;

/// Configuration for spawning nodes
pub struct SpawnConfig {
    /// Whether to load external assets (models, etc.)
    /// Set to false for editor preview, true for runtime
    pub load_assets: bool,
}

impl Default for SpawnConfig {
    fn default() -> Self {
        Self { load_assets: true }
    }
}

/// Spawn the game components for a node based on its type.
/// This adds only the gameplay-relevant components (meshes, lights, etc.)
/// The caller is responsible for adding editor-specific components if needed.
///
/// Returns the entity that was passed in for chaining.
pub fn spawn_node_components(
    entity_commands: &mut EntityCommands,
    node_data: &NodeData,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: Option<&AssetServer>,
    _config: &SpawnConfig,
) {
    match node_data.node_type.as_str() {
        // Camera nodes
        "camera.camera3d" => {
            spawn_camera_3d(entity_commands, &node_data.data);
        }

        // Mesh primitives
        "mesh.cube" => {
            spawn_mesh_primitive(entity_commands, MeshPrimitiveType::Cube, meshes, materials);
        }
        "mesh.sphere" => {
            spawn_mesh_primitive(entity_commands, MeshPrimitiveType::Sphere, meshes, materials);
        }
        "mesh.cylinder" => {
            spawn_mesh_primitive(entity_commands, MeshPrimitiveType::Cylinder, meshes, materials);
        }
        "mesh.plane" => {
            spawn_mesh_primitive(entity_commands, MeshPrimitiveType::Plane, meshes, materials);
        }

        // Mesh instance (3D model)
        "mesh.instance" => {
            spawn_mesh_instance(entity_commands, &node_data.data, asset_server);
        }

        // Lights
        "light.point" => {
            spawn_point_light(entity_commands, &node_data.data);
        }
        "light.directional" => {
            spawn_directional_light(entity_commands, &node_data.data);
        }
        "light.spot" => {
            spawn_spot_light(entity_commands, &node_data.data);
        }

        // Scene roots and empty nodes (no additional components needed)
        "scene.3d" | "scene.2d" | "scene.ui" | "scene.other" | "node.empty" => {}

        // Physics components
        "physics.rigidbody3d" | "physics.staticbody3d" | "physics.kinematicbody3d" => {
            spawn_physics_body(entity_commands, &node_data.data);
        }
        "physics.collision_box" | "physics.collision_sphere" | "physics.collision_capsule"
        | "physics.collision_cylinder" => {
            spawn_collision_shape(entity_commands, &node_data.data);
        }

        // Scene instance
        "scene.instance" => {
            spawn_scene_instance(entity_commands, &node_data.data);
        }

        _ => {
            warn!("Unknown node type: {}", node_data.node_type);
        }
    }
}

// ============================================================================
// Camera spawning
// ============================================================================

fn spawn_camera_3d(entity_commands: &mut EntityCommands, data: &HashMap<String, serde_json::Value>) {
    let fov = data
        .get("fov")
        .and_then(|v| v.as_f64())
        .unwrap_or(45.0) as f32;
    let is_default = data
        .get("is_default_camera")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    entity_commands.insert(CameraNodeData {
        fov,
        is_default_camera: is_default,
    });
}

// ============================================================================
// Mesh spawning
// ============================================================================

fn spawn_mesh_primitive(
    entity_commands: &mut EntityCommands,
    mesh_type: MeshPrimitiveType,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let mesh = match mesh_type {
        MeshPrimitiveType::Cube => meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        MeshPrimitiveType::Sphere => meshes.add(Sphere::new(0.5)),
        MeshPrimitiveType::Cylinder => meshes.add(Cylinder::new(0.5, 1.0)),
        MeshPrimitiveType::Plane => meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(0.5))),
    };

    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.8, 0.8),
        ..default()
    });

    entity_commands.insert((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        MeshNodeData { mesh_type },
    ));
}

fn spawn_mesh_instance(
    entity_commands: &mut EntityCommands,
    data: &HashMap<String, serde_json::Value>,
    asset_server: Option<&AssetServer>,
) {
    let model_path = data
        .get("model_path")
        .and_then(|v| {
            if let Some(s) = v.as_str() {
                return Some(s.to_string());
            }
            serde_json::from_value::<String>(v.clone()).ok()
        });

    // Load the model if we have an asset server and a path
    if let (Some(asset_server), Some(ref path)) = (asset_server, &model_path) {
        if !path.is_empty() {
            // Strip "assets/" prefix if present - Bevy's asset server already looks in assets/
            let load_path = if path.starts_with("assets/") || path.starts_with("assets\\") {
                &path[7..]
            } else {
                path.as_str()
            };
            let scene_handle: Handle<Scene> = asset_server.load(format!("{}#Scene0", load_path));
            entity_commands.insert(SceneRoot(scene_handle));
        }
    }

    entity_commands.insert(MeshInstanceData { model_path });
}

// ============================================================================
// Light spawning
// ============================================================================

fn parse_color(data: &HashMap<String, serde_json::Value>, key: &str) -> Color {
    let arr = data.get(key).and_then(|v| v.as_array());
    if let Some(arr) = arr {
        let r = arr.get(0).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
        let g = arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
        let b = arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
        Color::srgb(r, g, b)
    } else {
        Color::WHITE
    }
}

fn spawn_point_light(entity_commands: &mut EntityCommands, data: &HashMap<String, serde_json::Value>) {
    let intensity = data
        .get("intensity")
        .and_then(|v| v.as_f64())
        .unwrap_or(1000.0) as f32;
    let color = parse_color(data, "color");
    let range = data
        .get("range")
        .and_then(|v| v.as_f64())
        .unwrap_or(20.0) as f32;

    entity_commands.insert(PointLight {
        intensity,
        color,
        range,
        shadows_enabled: true,
        ..default()
    });
}

fn spawn_directional_light(entity_commands: &mut EntityCommands, data: &HashMap<String, serde_json::Value>) {
    let illuminance = data
        .get("illuminance")
        .and_then(|v| v.as_f64())
        .unwrap_or(10000.0) as f32;
    let color = parse_color(data, "color");

    entity_commands.insert(DirectionalLight {
        illuminance,
        color,
        shadows_enabled: true,
        ..default()
    });
}

fn spawn_spot_light(entity_commands: &mut EntityCommands, data: &HashMap<String, serde_json::Value>) {
    let intensity = data
        .get("intensity")
        .and_then(|v| v.as_f64())
        .unwrap_or(1000.0) as f32;
    let color = parse_color(data, "color");
    let range = data
        .get("range")
        .and_then(|v| v.as_f64())
        .unwrap_or(20.0) as f32;
    let inner_angle = data
        .get("inner_angle")
        .and_then(|v| v.as_f64())
        .unwrap_or(30.0) as f32;
    let outer_angle = data
        .get("outer_angle")
        .and_then(|v| v.as_f64())
        .unwrap_or(45.0) as f32;

    entity_commands.insert(SpotLight {
        intensity,
        color,
        range,
        inner_angle: inner_angle.to_radians(),
        outer_angle: outer_angle.to_radians(),
        shadows_enabled: true,
        ..default()
    });
}

// ============================================================================
// Physics spawning
// ============================================================================

fn spawn_physics_body(entity_commands: &mut EntityCommands, data: &HashMap<String, serde_json::Value>) {
    if let Ok(physics_data) = serde_json::from_value::<PhysicsBodyData>(
        serde_json::to_value(data).unwrap_or_default(),
    ) {
        entity_commands.insert(physics_data);
    }
}

fn spawn_collision_shape(entity_commands: &mut EntityCommands, data: &HashMap<String, serde_json::Value>) {
    if let Ok(collision_data) = serde_json::from_value::<CollisionShapeData>(
        serde_json::to_value(data).unwrap_or_default(),
    ) {
        entity_commands.insert(collision_data);
    }
}

// ============================================================================
// Scene instance spawning
// ============================================================================

fn spawn_scene_instance(entity_commands: &mut EntityCommands, data: &HashMap<String, serde_json::Value>) {
    let scene_path = data
        .get("scene_path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    entity_commands.insert(SceneInstanceData {
        scene_path,
        is_open: false,
    });
    // TODO: Load nested scene
}
