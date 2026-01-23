use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Project configuration stored in project.toml
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProjectConfig {
    pub name: String,
    pub version: String,
    pub main_scene: String,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            name: "New Project".to_string(),
            version: "0.1.0".to_string(),
            main_scene: "scenes/main.scene".to_string(),
        }
    }
}

/// Runtime resource holding the currently open project
#[derive(Resource, Clone, Debug)]
pub struct CurrentProject {
    /// Path to the project directory
    pub path: PathBuf,
    /// Loaded project configuration
    pub config: ProjectConfig,
}

impl CurrentProject {
    /// Get the full path to a file relative to the project
    pub fn resolve_path(&self, relative: &str) -> PathBuf {
        self.path.join(relative)
    }

    /// Get the full path to the main scene
    pub fn main_scene_path(&self) -> PathBuf {
        self.resolve_path(&self.config.main_scene)
    }
}

/// Create a new project at the specified path
pub fn create_project(path: &Path, name: &str) -> Result<CurrentProject, Box<dyn std::error::Error>> {
    // Create project directory structure
    std::fs::create_dir_all(path)?;
    std::fs::create_dir_all(path.join("scenes"))?;
    std::fs::create_dir_all(path.join("assets"))?;
    std::fs::create_dir_all(path.join("plugins"))?;

    // Create project config
    let config = ProjectConfig {
        name: name.to_string(),
        version: "0.1.0".to_string(),
        main_scene: "scenes/main.scene".to_string(),
    };

    // Write project.toml
    let config_path = path.join("project.toml");
    let config_content = toml::to_string_pretty(&config)?;
    std::fs::write(&config_path, config_content)?;

    // Create empty main scene - user will add root node
    let scene_content = r#"SceneData(
    name: "Main Scene",
    root_nodes: [],
    editor_camera: (
        orbit_focus: (0.0, 0.0, 0.0),
        orbit_distance: 10.0,
        orbit_yaw: 0.3,
        orbit_pitch: 0.4,
    ),
)
"#;
    let scene_path = path.join("scenes").join("main.scene");
    std::fs::write(&scene_path, scene_content)?;

    Ok(CurrentProject {
        path: path.to_path_buf(),
        config,
    })
}

/// Open an existing project from project.toml path
pub fn open_project(project_toml_path: &Path) -> Result<CurrentProject, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(project_toml_path)?;
    let config: ProjectConfig = toml::from_str(&content)?;

    let path = project_toml_path
        .parent()
        .ok_or("Invalid project path")?
        .to_path_buf();

    Ok(CurrentProject { path, config })
}
