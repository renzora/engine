//! Runtime scene loader
//!
//! Loads and spawns scenes without editor-specific components.
//! Supports loading from embedded pack files or loose files.

use bevy::prelude::*;
use std::fs;
use std::path::PathBuf;

use super::pack::PackReader;
use super::shared::{
    CameraNodeData, CollisionShapeData, MeshInstanceData, MeshNodeData, MeshPrimitiveType,
    NodeData, PhysicsBodyData, SceneData, SceneInstanceData,
};

/// Plugin to handle runtime scene loading
pub struct RuntimeLoaderPlugin;

impl Plugin for RuntimeLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (extract_pack_assets, load_main_scene).chain());
    }
}

/// Resource containing the project configuration
#[derive(Resource)]
pub struct RuntimeProject {
    pub name: String,
    pub main_scene: String,
    pub project_path: PathBuf,
}

impl Default for RuntimeProject {
    fn default() -> Self {
        Self {
            name: "Untitled".to_string(),
            main_scene: "scenes/main.scene".to_string(),
            project_path: PathBuf::from("."),
        }
    }
}

/// Resource to track if we're using a pack file
#[derive(Resource, Default)]
pub struct PackMode {
    pub is_packed: bool,
    pub extract_dir: Option<PathBuf>,
}

/// Extract assets from embedded pack file if present
fn extract_pack_assets(mut commands: Commands) {
    // Try to open pack from current exe
    let pack_mode = if let Some(mut pack) = PackReader::from_current_exe() {
        info!("Detected embedded pack file, extracting assets...");

        // Get temp directory for extraction
        let extract_dir = std::env::temp_dir().join("renzora_runtime");

        // Clean up old extraction if exists
        let _ = fs::remove_dir_all(&extract_dir);

        // Extract all files
        let files = pack.list_files().iter().map(|s| s.to_string()).collect::<Vec<_>>();
        for file_path in files {
            if let Some(data) = pack.read(&file_path) {
                let dest_path = extract_dir.join(&file_path);

                // Create parent directories
                if let Some(parent) = dest_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }

                // Write file
                if let Err(e) = fs::write(&dest_path, &data) {
                    error!("Failed to extract {}: {}", file_path, e);
                } else {
                    info!("Extracted: {}", file_path);
                }
            }
        }

        // Change working directory to extracted location so asset server finds files
        if let Err(e) = std::env::set_current_dir(&extract_dir) {
            error!("Failed to change to extract directory: {}", e);
        }

        PackMode {
            is_packed: true,
            extract_dir: Some(extract_dir),
        }
    } else {
        info!("No embedded pack detected, loading from filesystem");
        PackMode::default()
    };

    commands.insert_resource(pack_mode);
}

/// Load the main scene from project.toml
pub fn load_main_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    // Try to load project.toml
    let project_path = PathBuf::from(".");
    let project_toml_path = project_path.join("project.toml");

    let (project_name, main_scene_path) = if project_toml_path.exists() {
        match fs::read_to_string(&project_toml_path) {
            Ok(content) => {
                let config: toml::Value = match toml::from_str(&content) {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Failed to parse project.toml: {}", e);
                        return;
                    }
                };
                let name = config
                    .get("project")
                    .and_then(|p| p.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("Untitled")
                    .to_string();
                let scene = config
                    .get("project")
                    .and_then(|p| p.get("main_scene"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("scenes/main.scene")
                    .to_string();
                (name, project_path.join(&scene))
            }
            Err(e) => {
                error!("Failed to read project.toml: {}", e);
                ("Untitled".to_string(), project_path.join("scenes/main.scene"))
            }
        }
    } else {
        warn!("No project.toml found, using defaults");
        ("Untitled".to_string(), project_path.join("scenes/main.scene"))
    };

    info!("Loading project '{}' with main scene: {:?}", project_name, main_scene_path);

    // Insert project resource
    commands.insert_resource(RuntimeProject {
        name: project_name,
        main_scene: main_scene_path.to_string_lossy().to_string(),
        project_path,
    });

    // Load the scene file
    if main_scene_path.exists() {
        match load_scene_file(&main_scene_path, &mut commands, &mut meshes, &mut materials, &asset_server) {
            Ok(()) => info!("Scene loaded successfully"),
            Err(e) => error!("Failed to load scene: {}", e),
        }
    } else {
        error!("Main scene file not found: {:?}", main_scene_path);
    }

    // Add ambient light
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 200.0,
        ..default()
    });
}

/// Load a scene file and spawn all entities
fn load_scene_file(
    path: &PathBuf,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
) -> Result<(), String> {
    info!("Attempting to load scene from: {:?}", path);

    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;
    info!("Scene file content length: {} bytes", content.len());

    let scene_data: SceneData =
        ron::from_str(&content).map_err(|e| format!("Failed to parse scene: {}", e))?;

    info!("Loaded scene: {} with {} root nodes", scene_data.name, scene_data.root_nodes.len());

    // Spawn all root nodes
    for node in &scene_data.root_nodes {
        spawn_node_recursive(commands, meshes, materials, asset_server, node, None);
    }

    Ok(())
}

/// Spawn a node and all its children recursively
fn spawn_node_recursive(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
    node: &NodeData,
    parent: Option<Entity>,
) -> Entity {
    let transform: Transform = node.transform.clone().into();

    // Create the base entity
    let mut entity_commands = commands.spawn((
        transform,
        Visibility::default(),
        Name::new(node.name.clone()),
    ));

    // Add parent relationship if this isn't a root node
    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    // Add type-specific components based on node_type
    match node.node_type.as_str() {
        // Camera nodes
        "camera.camera3d" => {
            let fov = node
                .data
                .get("fov")
                .and_then(|v| v.as_f64())
                .unwrap_or(45.0) as f32;
            let is_default = node
                .data
                .get("is_default_camera")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            entity_commands.insert(CameraNodeData {
                fov,
                is_default_camera: is_default,
            });
        }

        // Mesh primitives
        "mesh.cube" => {
            entity_commands.insert((
                Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.8, 0.8, 0.8),
                    ..default()
                })),
                MeshNodeData {
                    mesh_type: MeshPrimitiveType::Cube,
                },
            ));
        }
        "mesh.sphere" => {
            entity_commands.insert((
                Mesh3d(meshes.add(Sphere::new(0.5))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.8, 0.8, 0.8),
                    ..default()
                })),
                MeshNodeData {
                    mesh_type: MeshPrimitiveType::Sphere,
                },
            ));
        }
        "mesh.cylinder" => {
            entity_commands.insert((
                Mesh3d(meshes.add(Cylinder::new(0.5, 1.0))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.8, 0.8, 0.8),
                    ..default()
                })),
                MeshNodeData {
                    mesh_type: MeshPrimitiveType::Cylinder,
                },
            ));
        }
        "mesh.plane" => {
            entity_commands.insert((
                Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(0.5)))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.8, 0.8, 0.8),
                    ..default()
                })),
                MeshNodeData {
                    mesh_type: MeshPrimitiveType::Plane,
                },
            ));
        }

        // Mesh instance (3D model)
        "mesh.instance" => {
            // Debug: log the raw data
            info!("mesh.instance data: {:?}", node.data);

            let model_path = node
                .data
                .get("model_path")
                .and_then(|v| {
                    // Try as_str first (for JSON strings)
                    if let Some(s) = v.as_str() {
                        return Some(s.to_string());
                    }
                    // Try to deserialize as String (for other formats)
                    serde_json::from_value::<String>(v.clone()).ok()
                });

            info!("Resolved model_path: {:?}", model_path);

            if let Some(ref path) = model_path {
                info!("Loading model from path: {}", path);
                let scene_handle: Handle<Scene> = asset_server.load(format!("{}#Scene0", path));
                entity_commands.insert(SceneRoot(scene_handle));
            } else {
                warn!("No model_path found for mesh.instance");
            }

            entity_commands.insert(MeshInstanceData { model_path });
        }

        // Lights
        "light.point" => {
            let intensity = node
                .data
                .get("intensity")
                .and_then(|v| v.as_f64())
                .unwrap_or(1000.0) as f32;
            let color_arr = node
                .data
                .get("color")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    [
                        arr.get(0).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                        arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                        arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                    ]
                })
                .unwrap_or([1.0, 1.0, 1.0]);
            let range = node
                .data
                .get("range")
                .and_then(|v| v.as_f64())
                .unwrap_or(20.0) as f32;

            entity_commands.insert(PointLight {
                intensity,
                color: Color::srgb(color_arr[0], color_arr[1], color_arr[2]),
                range,
                shadows_enabled: true,
                ..default()
            });
        }
        "light.directional" => {
            let illuminance = node
                .data
                .get("illuminance")
                .and_then(|v| v.as_f64())
                .unwrap_or(10000.0) as f32;
            let color_arr = node
                .data
                .get("color")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    [
                        arr.get(0).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                        arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                        arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                    ]
                })
                .unwrap_or([1.0, 1.0, 1.0]);

            entity_commands.insert(DirectionalLight {
                illuminance,
                color: Color::srgb(color_arr[0], color_arr[1], color_arr[2]),
                shadows_enabled: true,
                ..default()
            });
        }
        "light.spot" => {
            let intensity = node
                .data
                .get("intensity")
                .and_then(|v| v.as_f64())
                .unwrap_or(1000.0) as f32;
            let color_arr = node
                .data
                .get("color")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    [
                        arr.get(0).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                        arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                        arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                    ]
                })
                .unwrap_or([1.0, 1.0, 1.0]);
            let range = node
                .data
                .get("range")
                .and_then(|v| v.as_f64())
                .unwrap_or(20.0) as f32;
            let inner_angle = node
                .data
                .get("inner_angle")
                .and_then(|v| v.as_f64())
                .unwrap_or(30.0) as f32;
            let outer_angle = node
                .data
                .get("outer_angle")
                .and_then(|v| v.as_f64())
                .unwrap_or(45.0) as f32;

            entity_commands.insert(SpotLight {
                intensity,
                color: Color::srgb(color_arr[0], color_arr[1], color_arr[2]),
                range,
                inner_angle: inner_angle.to_radians(),
                outer_angle: outer_angle.to_radians(),
                shadows_enabled: true,
                ..default()
            });
        }

        // Scene roots (just containers)
        "scene.3d" | "scene.2d" | "scene.ui" | "scene.other" | "node.empty" => {
            // No additional components needed
        }

        // Physics components (stored as data but not simulated without physics engine)
        "physics.rigidbody3d" | "physics.staticbody3d" | "physics.kinematicbody3d" => {
            // Store physics data for future physics engine integration
            if let Ok(physics_data) = serde_json::from_value::<PhysicsBodyData>(
                serde_json::to_value(&node.data).unwrap_or_default(),
            ) {
                entity_commands.insert(physics_data);
            }
        }
        "physics.collision_box" | "physics.collision_sphere" | "physics.collision_capsule"
        | "physics.collision_cylinder" => {
            // Store collision shape data for future physics engine integration
            if let Ok(collision_data) = serde_json::from_value::<CollisionShapeData>(
                serde_json::to_value(&node.data).unwrap_or_default(),
            ) {
                entity_commands.insert(collision_data);
            }
        }

        // Scene instance
        "scene.instance" => {
            let scene_path = node
                .data
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

        _ => {
            warn!("Unknown node type: {}", node.node_type);
        }
    }

    let entity = entity_commands.id();

    // Spawn children recursively
    for child in &node.children {
        spawn_node_recursive(commands, meshes, materials, asset_server, child, Some(entity));
    }

    entity
}
