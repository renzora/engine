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
            main_scene: "scenes/main.ron".to_string(),
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

    /// Convert an absolute path to a project-relative path string using forward slashes.
    /// If the path is already relative, returns it as-is (with forward slashes).
    /// If the path is absolute but not inside the project, returns None.
    pub fn make_relative(&self, path: &Path) -> Option<String> {
        let path_buf = if path.is_relative() {
            return Some(path.to_string_lossy().replace('\\', "/"));
        } else {
            path.to_path_buf()
        };

        // Try with canonicalized paths first for symlink/case handling
        let canonical_project = self.path.canonicalize().ok();
        let canonical_path = path_buf.canonicalize().ok();

        if let (Some(proj), Some(p)) = (&canonical_project, &canonical_path) {
            if let Ok(rel) = p.strip_prefix(proj) {
                return Some(rel.to_string_lossy().replace('\\', "/"));
            }
        }

        // Fall back to direct prefix stripping
        if let Ok(rel) = path_buf.strip_prefix(&self.path) {
            return Some(rel.to_string_lossy().replace('\\', "/"));
        }

        None
    }

    /// Copy an engine default asset into the project if it doesn't already exist.
    /// `engine_relative_path` is a path like "assets/materials/checkerboard_default.material_bp"
    /// relative to the engine CWD.
    /// Returns the project-relative path string on success.
    pub fn ensure_default_asset(&self, engine_relative_path: &str) -> Option<String> {
        let project_dest = self.path.join(engine_relative_path);

        // Already exists in the project — just return the relative path
        if project_dest.exists() {
            return Some(engine_relative_path.replace('\\', "/"));
        }

        // Source: relative to engine CWD
        let engine_source = PathBuf::from(engine_relative_path);
        if !engine_source.exists() {
            warn!("Engine default asset not found: {}", engine_relative_path);
            return None;
        }

        // Create parent directories
        if let Some(parent) = project_dest.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                error!("Failed to create directory {}: {}", parent.display(), e);
                return None;
            }
        }

        // Copy the file
        if let Err(e) = std::fs::copy(&engine_source, &project_dest) {
            error!("Failed to copy default asset to project: {}", e);
            return None;
        }

        info!("Copied default asset to project: {}", project_dest.display());
        Some(engine_relative_path.replace('\\', "/"))
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
        main_scene: "scenes/main.ron".to_string(),
    };

    // Write project.toml
    let config_path = path.join("project.toml");
    let config_content = toml::to_string_pretty(&config)?;
    std::fs::write(&config_path, config_content)?;

    // Create empty main scene using Bevy DynamicScene format
    // Editor metadata will be added when the user first saves the scene
    let scene_content = r#"(
  resources: {},
  entities: {},
)
"#;
    let scene_path = path.join("scenes").join("main.ron");
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
