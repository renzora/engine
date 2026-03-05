use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Window configuration for exported/runtime games
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WindowConfig {
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub fullscreen: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            resizable: true,
            fullscreen: false,
        }
    }
}

/// Project configuration stored in project.toml
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProjectConfig {
    pub name: String,
    pub version: String,
    pub main_scene: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default)]
    pub window: WindowConfig,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            name: "New Project".to_string(),
            version: "0.1.0".to_string(),
            main_scene: "scenes/main.ron".to_string(),
            icon: None,
            window: WindowConfig::default(),
        }
    }
}

/// Runtime resource holding the currently open project
#[derive(Resource, Clone, Debug)]
pub struct CurrentProject {
    pub path: PathBuf,
    pub config: ProjectConfig,
}

impl CurrentProject {
    pub fn resolve_path(&self, relative: &str) -> PathBuf {
        self.path.join(relative)
    }

    pub fn main_scene_path(&self) -> PathBuf {
        self.resolve_path(&self.config.main_scene)
    }

    pub fn make_relative(&self, path: &Path) -> Option<String> {
        if path.is_relative() {
            return Some(path.to_string_lossy().replace('\\', "/"));
        }

        let canonical_project = self.path.canonicalize().ok();
        let canonical_path = path.canonicalize().ok();

        if let (Some(proj), Some(p)) = (&canonical_project, &canonical_path) {
            if let Ok(rel) = p.strip_prefix(proj) {
                return Some(rel.to_string_lossy().replace('\\', "/"));
            }
        }

        if let Ok(rel) = path.strip_prefix(&self.path) {
            return Some(rel.to_string_lossy().replace('\\', "/"));
        }

        None
    }
}

/// Marker component for the editor's scene-navigation camera.
///
/// This camera is used for orbit/pan/zoom during editing and renders to the
/// viewport texture. It is hidden from the hierarchy and cannot be deleted.
/// User-created scene cameras are separate entities.
#[derive(Component)]
pub struct EditorCamera;

/// Marker component to hide an entity (and its children) from the hierarchy panel.
#[derive(Component)]
pub struct HideInHierarchy;

/// Marker component — entity is locked from editing in the hierarchy.
#[derive(Component)]
pub struct EditorLocked;

/// Serializable marker for a scene camera entity.
///
/// Stored alongside `Camera3d` so the camera can be recreated on scene load
/// (since `Camera3d` itself is not serializable).
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SceneCamera;

/// Mesh primitive type — serializable record of what shape an entity uses.
///
/// Stored alongside `Mesh3d` so the shape can be recreated on scene load.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub enum MeshPrimitive {
    Cube,
    Sphere,
    Plane { width: f32, height: f32 },
    Cylinder,
}

/// Base color for an entity's material — serializable companion to `MeshMaterial3d`.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct MeshColor(pub Color);

/// Marker resource requesting a scene save.
///
/// Insert this resource to trigger the scene save system next frame.
#[derive(Resource)]
pub struct SaveSceneRequested;

/// Holds the optional render target for the game camera.
///
/// - `Some(handle)` — camera renders to this image (editor mode).
/// - `None` — camera renders to the window (standalone mode).
#[derive(Resource, Default)]
pub struct ViewportRenderTarget {
    pub image: Option<Handle<Image>>,
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
