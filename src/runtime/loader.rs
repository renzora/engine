//! Runtime scene loader
//!
//! Loads and spawns scenes without editor-specific components.
//! Supports loading from embedded pack files or loose files.
//! Uses shared spawner to ensure consistency with editor.

use bevy::prelude::*;
use std::fs;
use std::path::PathBuf;

use super::pack_asset_reader::PackIndex;
use super::shared::{spawn_node_components, NodeData, SceneData, SpawnConfig};

/// Plugin to handle runtime scene loading
pub struct RuntimeLoaderPlugin;

impl Plugin for RuntimeLoaderPlugin {
    fn build(&self, app: &mut App) {
        // Note: Pack extraction now happens in main() before Bevy initializes
        app.add_systems(Startup, load_main_scene);
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

/// Find the project path by checking multiple locations
fn find_project_path() -> PathBuf {
    // 1. Check directory containing the executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let project_toml = exe_dir.join("project.toml");
            if project_toml.exists() {
                info!("Found project.toml next to executable: {:?}", exe_dir);
                return exe_dir.to_path_buf();
            }
        }
    }

    // 2. Check current working directory
    let cwd = PathBuf::from(".");
    let project_toml = cwd.join("project.toml");
    if project_toml.exists() {
        info!("Found project.toml in current directory");
        return cwd;
    }

    // 3. Check if there's a command line argument for project path
    if let Some(arg) = std::env::args().nth(1) {
        let arg_path = PathBuf::from(&arg);
        let project_toml = arg_path.join("project.toml");
        if project_toml.exists() {
            info!("Found project.toml from command line argument: {:?}", arg_path);
            return arg_path;
        }
    }

    // Default to current directory
    warn!("No project.toml found, using current directory");
    PathBuf::from(".")
}

/// Load the main scene from project.toml
pub fn load_main_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    pack_index: Option<Res<PackIndex>>,
) {
    // Get project config from pack or filesystem
    let (project_name, main_scene_rel) = if let Some(ref pack) = pack_index {
        // Read from pack
        if let Some(content) = pack.read_string("project.toml") {
            match toml::from_str::<toml::Value>(&content) {
                Ok(config) => {
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
                    (name, scene)
                }
                Err(e) => {
                    error!("Failed to parse project.toml from pack: {}", e);
                    ("Untitled".to_string(), "scenes/main.scene".to_string())
                }
            }
        } else {
            error!("project.toml not found in pack");
            ("Untitled".to_string(), "scenes/main.scene".to_string())
        }
    } else {
        // Read from filesystem
        let project_path = find_project_path();
        info!("Using project path: {:?}", project_path);

        let project_toml_path = project_path.join("project.toml");
        if project_toml_path.exists() {
            match fs::read_to_string(&project_toml_path) {
                Ok(content) => {
                    match toml::from_str::<toml::Value>(&content) {
                        Ok(config) => {
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
                            (name, scene)
                        }
                        Err(e) => {
                            error!("Failed to parse project.toml: {}", e);
                            ("Untitled".to_string(), "scenes/main.scene".to_string())
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to read project.toml: {}", e);
                    ("Untitled".to_string(), "scenes/main.scene".to_string())
                }
            }
        } else {
            warn!("No project.toml found, using defaults");
            ("Untitled".to_string(), "scenes/main.scene".to_string())
        }
    };

    info!("Loading project '{}' with main scene: {}", project_name, main_scene_rel);

    // Insert project resource
    commands.insert_resource(RuntimeProject {
        name: project_name.clone(),
        main_scene: main_scene_rel.clone(),
        project_path: PathBuf::from("."),
    });

    // Load the scene file from pack or filesystem
    let scene_result = if let Some(ref pack) = pack_index {
        // Read scene from pack
        let scene_path = main_scene_rel.replace('\\', "/");
        info!("Loading scene from pack: {}", scene_path);
        if let Some(content) = pack.read_string(&scene_path) {
            load_scene_from_string(&content, &mut commands, &mut meshes, &mut materials, &asset_server)
        } else {
            Err(format!("Scene file not found in pack: {}", scene_path))
        }
    } else {
        // Read scene from filesystem
        let project_path = find_project_path();
        let main_scene_path = project_path.join(&main_scene_rel);
        info!("Loading scene from filesystem: {:?}", main_scene_path);
        if main_scene_path.exists() {
            load_scene_file(&main_scene_path, &mut commands, &mut meshes, &mut materials, &asset_server)
        } else {
            Err(format!("Scene file not found: {:?}", main_scene_path))
        }
    };

    match scene_result {
        Ok(()) => info!("Scene loaded successfully"),
        Err(e) => error!("Failed to load scene: {}", e),
    }

    // Add ambient light
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 200.0,
        ..default()
    });
}

/// Load a scene from a string (for pack files)
fn load_scene_from_string(
    content: &str,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
) -> Result<(), String> {
    info!("Scene content length: {} bytes", content.len());

    let scene_data: SceneData =
        ron::from_str(content).map_err(|e| format!("Failed to parse scene: {}", e))?;

    info!("Loaded scene: {} with {} root nodes", scene_data.name, scene_data.root_nodes.len());

    // Spawn all root nodes
    for node in &scene_data.root_nodes {
        spawn_node_recursive(commands, meshes, materials, asset_server, node, None);
    }

    Ok(())
}

/// Load a scene file from filesystem and spawn all entities
fn load_scene_file(
    path: &PathBuf,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
) -> Result<(), String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;
    load_scene_from_string(&content, commands, meshes, materials, asset_server)
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

    // Create the base entity with transform, visibility, and name
    let mut entity_commands = commands.spawn((
        transform,
        Visibility::default(),
        Name::new(node.name.clone()),
    ));

    // Add parent relationship if this isn't a root node
    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    // Use shared spawner to add type-specific components
    let config = SpawnConfig::default();
    spawn_node_components(
        &mut entity_commands,
        node,
        meshes,
        materials,
        Some(asset_server),
        &config,
    );

    let entity = entity_commands.id();

    // Spawn children recursively
    for child in &node.children {
        spawn_node_recursive(commands, meshes, materials, asset_server, child, Some(entity));
    }

    entity
}
