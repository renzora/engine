//! Runtime scene loader
//!
//! Loads and spawns scenes using Bevy's DynamicScene system.
//! Supports loading from embedded pack files or loose files.

use bevy::prelude::*;
use bevy::scene::DynamicSceneRoot;
use std::fs;
use std::path::PathBuf;

use super::pack_asset_reader::PackIndex;

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
            main_scene: "scenes/main.ron".to_string(),
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
                        .unwrap_or("scenes/main.ron")
                        .to_string();
                    (name, scene)
                }
                Err(e) => {
                    error!("Failed to parse project.toml from pack: {}", e);
                    ("Untitled".to_string(), "scenes/main.ron".to_string())
                }
            }
        } else {
            error!("project.toml not found in pack");
            ("Untitled".to_string(), "scenes/main.ron".to_string())
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
                                .unwrap_or("scenes/main.ron")
                                .to_string();
                            (name, scene)
                        }
                        Err(e) => {
                            error!("Failed to parse project.toml: {}", e);
                            ("Untitled".to_string(), "scenes/main.ron".to_string())
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to read project.toml: {}", e);
                    ("Untitled".to_string(), "scenes/main.ron".to_string())
                }
            }
        } else {
            warn!("No project.toml found, using defaults");
            ("Untitled".to_string(), "scenes/main.ron".to_string())
        }
    };

    info!("Loading project '{}' with main scene: {}", project_name, main_scene_rel);

    // Insert project resource
    commands.insert_resource(RuntimeProject {
        name: project_name.clone(),
        main_scene: main_scene_rel.clone(),
        project_path: PathBuf::from("."),
    });

    // Load the scene using Bevy's DynamicScene system
    // The asset server will handle both pack files and filesystem
    let scene_handle: Handle<DynamicScene> = asset_server.load(&main_scene_rel);

    // Spawn the scene root - Bevy will automatically load and instantiate the scene
    commands.spawn(DynamicSceneRoot(scene_handle));

    info!("Loading scene: {}", main_scene_rel);

    // Add ambient light
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 200.0,
        ..default()
    });
}
