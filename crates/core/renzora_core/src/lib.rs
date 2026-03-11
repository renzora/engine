pub mod console_log;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Window configuration for exported/runtime games
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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

    /// Save the project config back to project.toml.
    pub fn save_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        let toml_path = self.path.join("project.toml");
        let content = toml::to_string_pretty(&self.config)?;
        std::fs::write(&toml_path, content)?;
        Ok(())
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

/// Unique tag for identifying an entity from scripts and other systems.
///
/// Unlike `Name` (which is a display label and can be duplicated), a tag
/// is intended to be a unique identifier for lookup via `get_on()` etc.
#[derive(Component, Default, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct EntityTag {
    pub tag: String,
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

/// Marker component — camera should be excluded from scene-wide effects (skybox, post-processing).
#[derive(Component)]
pub struct IsolatedCamera;

/// Serializable marker for a scene camera entity.
///
/// Stored alongside `Camera3d` so the camera can be recreated on scene load
/// (since `Camera3d` itself is not serializable).
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SceneCamera;

/// Marks a camera as the default game camera for preview and play mode.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct DefaultCamera;

/// Maps each rendering camera to its effect source entities.
///
/// Each route is `(target_camera, [source_entities])`. For a given Settings
/// type the **first** source entity that has it wins.
///
/// Updated each frame by the routing system (editor: viewport crate,
/// runtime: renzora_runtime). Read by per-crate sync systems.
#[derive(Resource, Default, Debug)]
pub struct EffectRouting {
    pub routes: Vec<(Entity, Vec<Entity>)>,
}

impl EffectRouting {
    /// Iterate all routes.
    pub fn iter(&self) -> impl Iterator<Item = &(Entity, Vec<Entity>)> {
        self.routes.iter()
    }
}

/// Serializable shape ID — stored alongside `Mesh3d` so the shape can be recreated on scene load.
///
/// The string must match a shape registered in the [`ShapeRegistry`].
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct MeshPrimitive(pub String);

// ============================================================================
// Shape Registry
// ============================================================================

/// A registered shape that can be spawned and rehydrated.
pub struct ShapeEntry {
    /// Unique identifier (e.g. `"cube"`, `"spiral_stairs"`). Must match `MeshPrimitive` values.
    pub id: &'static str,
    /// Human-readable name shown in UI.
    pub name: &'static str,
    /// Phosphor icon glyph for the shape library panel.
    pub icon: &'static str,
    /// Category string for grouping in UI (e.g. `"Basic"`, `"Level"`).
    pub category: &'static str,
    /// Factory function that creates the mesh.
    pub create_mesh: fn(&mut Assets<Mesh>) -> Handle<Mesh>,
    /// Default base color when spawning.
    pub default_color: Color,
}

/// Global registry of available shapes. Crates register shapes during plugin `build()`.
///
/// Used by the shape library panel (editor) and rehydration (runtime) to look up
/// mesh factories by ID.
#[derive(Resource, Default)]
pub struct ShapeRegistry {
    entries: Vec<ShapeEntry>,
}

impl ShapeRegistry {
    /// Register a new shape. Duplicate IDs are silently ignored.
    pub fn register(&mut self, entry: ShapeEntry) {
        if self.entries.iter().any(|e| e.id == entry.id) {
            return;
        }
        self.entries.push(entry);
    }

    /// Look up a shape by ID.
    pub fn get(&self, id: &str) -> Option<&ShapeEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Iterate over all registered shapes.
    pub fn iter(&self) -> impl Iterator<Item = &ShapeEntry> {
        self.entries.iter()
    }

    /// Create a mesh for the given shape ID, or `None` if not registered.
    pub fn create_mesh(&self, id: &str, meshes: &mut Assets<Mesh>) -> Option<Handle<Mesh>> {
        self.get(id).map(|entry| (entry.create_mesh)(meshes))
    }
}

/// Base color for an entity's material — serializable companion to `MeshMaterial3d`.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct MeshColor(pub Color);

// ============================================================================
// Play Mode
// ============================================================================

/// Current play-mode state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PlayState {
    /// Normal editing.
    #[default]
    Editing,
    /// Game is running (game camera active, editor overlays hidden).
    Playing,
    /// Game is paused.
    Paused,
    /// Scripts running inside the editor (editor UI visible, no camera switch).
    ScriptsOnly,
    /// Scripts paused inside the editor.
    ScriptsPaused,
}

/// Resource that tracks play mode state and pending transitions.
#[derive(Resource, Default)]
pub struct PlayModeState {
    pub state: PlayState,
    /// Entity of the active game camera during play mode.
    pub active_game_camera: Option<bevy::ecs::entity::Entity>,
    /// Set to `true` to request entering play mode next frame.
    pub request_play: bool,
    /// Set to `true` to request stopping play mode next frame.
    pub request_stop: bool,
    /// Set to `true` to toggle pause.
    pub request_pause: bool,
    /// Set to `true` to request scripts-only mode next frame.
    pub request_scripts_only: bool,
}

impl PlayModeState {
    pub fn is_playing(&self) -> bool {
        self.state == PlayState::Playing
    }
    pub fn is_paused(&self) -> bool {
        matches!(self.state, PlayState::Paused | PlayState::ScriptsPaused)
    }
    pub fn is_editing(&self) -> bool {
        self.state == PlayState::Editing
    }
    /// Returns true if in Playing or Paused state (full play mode).
    pub fn is_in_play_mode(&self) -> bool {
        matches!(self.state, PlayState::Playing | PlayState::Paused)
    }
    /// Returns true if scripts are in scripts-only mode (running or paused).
    pub fn is_scripts_only(&self) -> bool {
        matches!(self.state, PlayState::ScriptsOnly | PlayState::ScriptsPaused)
    }
    /// Returns true if scripts should be executing this frame.
    pub fn is_scripts_running(&self) -> bool {
        matches!(self.state, PlayState::Playing | PlayState::ScriptsOnly)
    }
}

/// Marker component added to the game camera entity during play mode.
#[derive(Component)]
pub struct PlayModeCamera;

/// Resource: request a scene load from scripts/blueprints.
/// The runtime system drains this each frame.
#[derive(Resource, Default)]
pub struct PendingSceneLoad {
    /// Scene name or relative path to load.
    pub requests: Vec<String>,
}

/// Marker resource requesting a scene save.
///
/// Insert this resource to trigger the scene save system next frame.
#[derive(Resource)]
pub struct SaveSceneRequested;

/// Request "Save As" — prompts user for a new scene name/path.
#[derive(Resource)]
pub struct SaveAsSceneRequested;

/// Request "New Scene" — clears the world and sets up a blank scene.
#[derive(Resource)]
pub struct NewSceneRequested;

/// Request "Open Scene" — prompts user to pick a scene file.
#[derive(Resource)]
pub struct OpenSceneRequested;

/// Marker resource requesting the export overlay to open.
///
/// Insert this resource to trigger the export overlay next frame.
#[derive(Resource)]
pub struct ExportRequested;

/// Marker resource requesting the tutorial overlay to start.
#[derive(Resource)]
pub struct TutorialRequested;

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
