pub mod console_log;
pub mod keybindings;
pub mod reflection;
pub mod viewport_types;

use bevy::prelude::*;
use bevy::input::gamepad::{GamepadAxis, GamepadButton};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Generic file reader resource that abstracts filesystem vs. archive (rpak) reads.
///
/// By default reads from disk. The runtime replaces this with a Vfs-backed
/// reader so materials (and other systems) transparently read from rpak archives.
#[derive(Resource, Clone)]
pub struct VirtualFileReader {
    reader: Arc<dyn Fn(&str) -> Option<String> + Send + Sync>,
}

impl Default for VirtualFileReader {
    fn default() -> Self {
        Self {
            reader: Arc::new(|path| std::fs::read_to_string(path).ok()),
        }
    }
}

impl VirtualFileReader {
    /// Create a reader backed by a custom function.
    pub fn new(f: impl Fn(&str) -> Option<String> + Send + Sync + 'static) -> Self {
        Self { reader: Arc::new(f) }
    }

    /// Read a file to string. Tries the backing store (archive or disk).
    pub fn read_string(&self, path: &str) -> Option<String> {
        (self.reader)(path)
    }
}

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

/// Network configuration stored in `[network]` section of project.toml.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NetworkProjectConfig {
    /// Server address (IP or hostname).
    #[serde(default = "default_server_addr")]
    pub server_addr: String,
    /// Port for the server to listen on / client to connect to.
    #[serde(default = "default_port")]
    pub port: u16,
    /// Transport protocol: "udp", "webtransport", "websocket".
    #[serde(default = "default_transport")]
    pub transport: String,
    /// Server tick rate in Hz.
    #[serde(default = "default_tick_rate")]
    pub tick_rate: u16,
    /// Maximum number of connected clients.
    #[serde(default = "default_max_clients")]
    pub max_clients: u16,
}

fn default_server_addr() -> String { "127.0.0.1".to_string() }
fn default_port() -> u16 { 7636 }
fn default_transport() -> String { "udp".to_string() }
fn default_tick_rate() -> u16 { 64 }
fn default_max_clients() -> u16 { 32 }

impl Default for NetworkProjectConfig {
    fn default() -> Self {
        Self {
            server_addr: default_server_addr(),
            port: default_port(),
            transport: default_transport(),
            tick_rate: default_tick_rate(),
            max_clients: default_max_clients(),
        }
    }
}

/// Project configuration stored in project.toml
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ProjectConfig {
    pub name: String,
    pub version: String,
    pub main_scene: String,
    /// The scene the editor had open when the project was last closed. Editor
    /// reopens this on project load, falling back to `main_scene` if absent.
    /// Runtime / exported builds always use `main_scene` (this field is
    /// editor-only and ignored by the runtime).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor_last_scene: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default)]
    pub window: WindowConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkProjectConfig>,
    /// Editor-only preferences (viewport toggles, camera speed, snap, etc.).
    /// The runtime ignores this section; export strips it from shipped builds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<crate::core::viewport_types::EditorPrefs>,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            name: "New Project".to_string(),
            version: "0.1.0".to_string(),
            main_scene: "scenes/main.ron".to_string(),
            editor_last_scene: None,
            icon: None,
            window: WindowConfig::default(),
            network: None,
            editor: None,
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

    /// Convert an absolute path to a project-relative path (e.g. `assets/textures/foo.png`).
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

    /// Convert an absolute path to an asset-relative path for `AssetServer::load()`.
    ///
    /// Strips the project root prefix so the resulting path
    /// (e.g. `textures/foo.png`) is portable across machines and works with the
    /// asset reader in both editor and standalone runtime builds.
    pub fn make_asset_relative(&self, path: &Path) -> String {
        // Try direct strip first
        if let Ok(rel) = path.strip_prefix(&self.path) {
            return rel.to_string_lossy().replace('\\', "/");
        }

        // Try canonicalized paths
        if let (Ok(canon_proj), Ok(canon_path)) =
            (self.path.canonicalize(), path.canonicalize())
        {
            if let Ok(rel) = canon_path.strip_prefix(&canon_proj) {
                return rel.to_string_lossy().replace('\\', "/");
            }
        }

        // Fallback: return the path as-is with normalized slashes
        path.to_string_lossy().replace('\\', "/")
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

/// Marker component — entity persists across scene loads (e.g. loader UI root).
/// `process_pending_scene_loads` and similar despawn-the-world logic must skip these.
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct Persistent;

/// Marker component — entity is locked from editing in the hierarchy.
#[derive(Component)]
pub struct EditorLocked;

/// Marker component — viewport picking stops at this entity instead of walking
/// past it to a higher-up named ancestor. Apply to compound entities (terrains,
/// prefab roots, etc.) that own many named children but should be selectable
/// as a unit.
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct SelectionStop;

/// Marker component — camera should be excluded from scene-wide effects (skybox, post-processing).
#[derive(Component)]
pub struct IsolatedCamera;

/// Marks an entity as the root of a nested-scene instance.
///
/// The `source` field is an asset-relative path to the `.ron` scene file that
/// provides the instance's contents. In the host scene file, only this root
/// entity (with its transform + any host-level overrides) is serialized; the
/// instance's child entity tree lives in the referenced source file and is
/// expanded on load.
///
/// Edits to entities *inside* an instance tree autosave back to the source
/// file. Edits to the instance root's transform persist in the host scene as
/// per-instance placement overrides.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SceneInstance {
    /// Asset-relative path to the source `.ron` scene file.
    pub source: String,
}

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
/// runtime: renzora_engine). Read by per-crate sync systems.
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

/// Event fired when a model importer has pulled PBR material data out of a
/// source file and needs somewhere to persist it as a `.material` graph.
/// `renzora_import_ui` triggers this per extracted material; a handler in
/// `renzora_material` (or any other provider) observes and writes the file.
/// Both sides communicate only through this type — no sibling crate deps.
#[derive(Event, Debug, Clone)]
pub struct PbrMaterialExtracted {
    /// Human-friendly name for the material; becomes the `.material` filename.
    pub name: String,
    /// Absolute path of the directory to write the `.material` file into.
    pub output_dir: std::path::PathBuf,
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    /// Asset-relative URI to the base-color texture (e.g.
    /// `"models/character/textures/diffuse.png"`), or `None`.
    pub base_color_texture: Option<String>,
    pub normal_texture: Option<String>,
}

/// Event fired when a file or folder is renamed/moved inside the project's
/// asset tree. Subscribers should patch any stored asset-relative references
/// from `old` to `new` (and, when `old` is a folder, any paths prefixed by it).
/// Paths are asset-relative (no leading project root, forward slashes).
#[derive(Event, Debug, Clone)]
pub struct AssetPathChanged {
    pub old: String,
    pub new: String,
    /// `true` when the moved item was a directory — consumers should perform
    /// prefix matching on stored paths. `false` matches the exact path.
    pub is_dir: bool,
}

impl AssetPathChanged {
    /// If `path` references the moved asset (or something under it when
    /// `is_dir`), return the rewritten path. Otherwise `None`.
    pub fn rewrite(&self, path: &str) -> Option<String> {
        if self.is_dir {
            if let Some(rest) = path.strip_prefix(&self.old) {
                let sep = rest.starts_with('/') || rest.is_empty();
                if sep {
                    return Some(format!("{}{}", self.new, rest));
                }
            }
            None
        } else if path == self.old {
            Some(self.new.clone())
        } else {
            None
        }
    }
}

/// Serializable marker for an imported 3D model (GLTF/GLB).
///
/// Stored on the parent entity; the actual `SceneRoot` is a child.
/// On scene load, the runtime rehydrates by re-loading the model from `model_path`.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct MeshInstanceData {
    /// Asset-relative path to the GLB/GLTF file (e.g. `models/chair.glb`).
    pub model_path: Option<String>,
}

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

    /// Look up a shape by ID (mutable).
    pub fn get_mut(&mut self, id: &str) -> Option<&mut ShapeEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
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
// Editor ↔ Physics decoupling events
// ============================================================================

/// Sent by the editor to request pausing the physics simulation.
#[derive(bevy::prelude::Event)]
pub struct PausePhysics;

/// Sent by the editor to request unpausing the physics simulation.
#[derive(bevy::prelude::Event)]
pub struct UnpausePhysics;

/// Sent by the editor to request resetting all script runtime states.
#[derive(bevy::prelude::Event)]
pub struct ResetScriptStates;

/// Notification that scripts were hot-reloaded. The scripting crate triggers
/// this so the editor can show toast notifications without importing scripting.
#[derive(bevy::prelude::Event)]
pub struct ScriptsReloaded {
    pub names: Vec<String>,
}

/// Sent by the editor to save the current scene before play mode.
#[derive(bevy::prelude::Event)]
pub struct SaveCurrentScene;

// ============================================================================
// Character Controller Commands (shared between scripting and physics)
// ============================================================================

/// Queued character controller commands, processed by renzora_physics each frame.
#[derive(bevy::prelude::Resource, Default)]
pub struct CharacterCommandQueue {
    pub commands: Vec<(bevy::ecs::entity::Entity, CharacterCommand)>,
}

/// A character controller command for a specific entity.
#[derive(Debug)]
pub enum CharacterCommand {
    Move(bevy::prelude::Vec2),
    Jump,
    Sprint(bool),
}

// ============================================================================
// Action State (shared between input and physics/scripting)
// ============================================================================

/// Per-action runtime state computed each frame by the input system.
#[derive(Clone, Debug, Default)]
pub struct ActionData {
    pub pressed: bool,
    pub just_pressed: bool,
    pub just_released: bool,
    pub axis_1d: f32,
    pub axis_2d: bevy::prelude::Vec2,
}

/// Computed action states, populated by the input system and read by
/// physics, scripting, and blueprints each frame.
#[derive(bevy::prelude::Resource, Clone, Debug, Default)]
pub struct ActionState {
    pub actions: std::collections::HashMap<String, ActionData>,
}

impl ActionState {
    pub fn pressed(&self, action: &str) -> bool {
        self.actions.get(action).map_or(false, |a| a.pressed)
    }
    pub fn just_pressed(&self, action: &str) -> bool {
        self.actions.get(action).map_or(false, |a| a.just_pressed)
    }
    pub fn just_released(&self, action: &str) -> bool {
        self.actions.get(action).map_or(false, |a| a.just_released)
    }
    pub fn axis_1d(&self, action: &str) -> f32 {
        self.actions.get(action).map_or(0.0, |a| a.axis_1d)
    }
    pub fn axis_2d(&self, action: &str) -> bevy::prelude::Vec2 {
        self.actions.get(action).map_or(bevy::prelude::Vec2::ZERO, |a| a.axis_2d)
    }
}

// ============================================================================
// MaterialRef (shared between material and terrain)
// ============================================================================

/// Reference to a material file. Add to any entity with `Mesh3d` to assign a material.
#[derive(bevy::prelude::Component, serde::Serialize, serde::Deserialize, bevy::prelude::Reflect, Clone, Debug)]
#[reflect(Component, Serialize, Deserialize)]
pub struct MaterialRef(pub String);

// ============================================================================
// Animation clip format (shared between animation and import)
// ============================================================================

/// One animation clip, serialized to a `.anim` file (RON format).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnimClip {
    pub name: String,
    pub duration: f32,
    pub tracks: Vec<BoneTrack>,
}

/// Animation curves for a single bone/target.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BoneTrack {
    pub bone_name: String,
    pub translations: Vec<(f32, [f32; 3])>,
    pub rotations: Vec<(f32, [f32; 4])>,
    pub scales: Vec<(f32, [f32; 3])>,
}

// ============================================================================
// TransformWrite (deferred transform mutations from scripts/blueprints)
// ============================================================================

/// Deferred transform write — batched and applied by the scripting command processor.
#[derive(Debug)]
pub struct TransformWrite {
    pub entity: bevy::ecs::entity::Entity,
    pub new_position: Option<bevy::prelude::Vec3>,
    pub new_rotation: Option<bevy::prelude::Vec3>,
    pub translation: Option<bevy::prelude::Vec3>,
    pub rotation_delta: Option<bevy::prelude::Vec3>,
    pub new_scale: Option<bevy::prelude::Vec3>,
    pub look_at: Option<bevy::prelude::Vec3>,
}

/// Queue for batched transform writes.
#[derive(bevy::prelude::Resource, Default)]
pub struct TransformWriteQueue {
    pub writes: Vec<TransformWrite>,
}

/// Write an AnimClip to a `.anim` file (RON format).
pub fn write_anim_file(clip: &AnimClip, path: &std::path::Path) -> Result<(), String> {
    let ron_str = ron::ser::to_string_pretty(clip, ron::ser::PrettyConfig::default())
        .map_err(|e| format!("RON serialization error: {}", e))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }
    std::fs::write(path, ron_str)
        .map_err(|e| format!("Failed to write file: {}", e))?;
    Ok(())
}

/// Generic script action event. Scripts call `action("name", { args })` and
/// domain crates observe this event to handle actions they recognize.
/// This decouples scripting from domain crates — no ScriptExtension imports needed.
#[derive(bevy::prelude::Event, Debug, Clone)]
pub struct ScriptAction {
    /// The action name (e.g. "apply_force", "play_sound", "gauge_damage").
    pub name: String,
    /// The entity that triggered the action (script's owning entity).
    pub entity: bevy::ecs::entity::Entity,
    /// Optional target entity (by name or ID).
    pub target_entity: Option<String>,
    /// Action arguments as key-value pairs.
    pub args: std::collections::HashMap<String, ScriptActionValue>,
}

/// Value types for script action arguments.
#[derive(Debug, Clone)]
pub enum ScriptActionValue {
    Float(f32),
    Int(i64),
    Bool(bool),
    String(String),
    Vec3([f32; 3]),
}

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

/// Run condition: returns true when NOT in play mode (editing or scripts-only).
/// Use as `.run_if(not_in_play_mode)` on editor systems that should be disabled during play.
pub fn not_in_play_mode(play_mode: Option<Res<PlayModeState>>) -> bool {
    !play_mode.as_ref().map_or(false, |pm| pm.is_in_play_mode())
}

/// Marker component added to the game camera entity during play mode.
#[derive(Component)]
pub struct PlayModeCamera;

/// Marker component for the UI canvas preview camera.
#[derive(Component)]
pub struct UiCanvasPreviewCamera;

/// Lightweight network status bridge — updated by the network crate,
/// read by blueprint and other crates that need connection info without
/// depending on renzora_network.
#[derive(Resource, Default, Clone, Debug)]
pub struct NetworkBridge {
    /// Whether this instance is running as a server.
    pub is_server: bool,
    /// Whether the client is connected to a server (or server is running).
    pub is_connected: bool,
    /// Number of connected clients (server only).
    pub player_count: i32,
}

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

/// Request a tab switch — serializes current scene, deserializes target.
#[derive(Resource)]
pub struct TabSwitchRequest {
    pub old_tab_id: u64,
    pub new_tab_id: u64,
}

/// In-memory snapshot of a scene tab's state (entities + camera).
pub struct TabSceneSnapshot {
    pub scene_ron: String,
    pub camera_focus: [f32; 3],
    pub camera_distance: f32,
    pub camera_yaw: f32,
    pub camera_pitch: f32,
}

/// Stores serialized scene data for each tab so switching tabs can serialize/deserialize.
#[derive(Resource, Default)]
pub struct SceneTabBuffers {
    pub buffers: std::collections::HashMap<u64, TabSceneSnapshot>,
}

/// Marker resource requesting the export overlay to open.
///
/// Insert this resource to trigger the export overlay next frame.
#[derive(Resource)]
pub struct ExportRequested;

/// Marker resource requesting the import overlay to open.
///
/// Insert this resource to trigger the import overlay next frame.
#[derive(Resource)]
pub struct ImportRequested;

/// Optional: carries the suggested target directory from the asset browser.
#[derive(Resource)]
pub struct ImportTargetDir(pub String);

/// Marker resource requesting the tutorial overlay to start.
#[derive(Resource)]
pub struct TutorialRequested;

/// One-shot: request to toggle the settings overlay.
#[derive(Resource)]
pub struct ToggleSettingsRequested;

/// One-shot: request to open the Create Node overlay in the hierarchy panel.
#[derive(Resource)]
pub struct CreateNodeRequested;

/// One-shot: request the code editor to open a file.
///
/// Inserted by the asset browser (or any plugin) so the code editor plugin
/// can observe it without a direct crate dependency.
#[derive(Resource)]
pub struct OpenCodeEditorFile {
    pub path: std::path::PathBuf,
}

/// Tracks whether a UI text input has keyboard focus.
///
/// When `true`, keyboard shortcuts should not fire so typing is not interrupted.
/// Updated each frame by the viewport/editor systems from egui state.
#[derive(Resource, Default)]
pub struct InputFocusState {
    pub egui_wants_keyboard: bool,
    /// True when the pointer is over an egui panel (not the viewport).
    pub egui_has_pointer: bool,
}

/// HUD data for the modal transform overlay (written by gizmo crate, read by viewport).
///
/// When `active` is true the viewport panel draws the scale circle / axis info.
#[derive(Resource, Default)]
pub struct ModalTransformHud {
    /// Whether modal transform is active.
    pub active: bool,
    /// Mode name ("Grab", "Rotate", "Scale").
    pub mode: &'static str,
    /// Whether this is Scale mode (draws circle + line overlay).
    pub is_scale: bool,
    /// Screen-space pivot position (entity center projected).
    pub pivot: Option<[f32; 2]>,
    /// Current cursor screen position.
    pub cursor: [f32; 2],
    /// Axis constraint name ("", "X", "Y", "Z", "YZ", "XZ", "XY").
    pub axis_name: &'static str,
    /// Axis constraint color [r, g, b, a] in 0..=255.
    pub axis_color: [u8; 4],
    /// Numeric input display string.
    pub numeric_display: String,
}

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

// ── Auth bridge ──────────────────────────────────────────────────────────────

/// Lightweight auth info resource that the auth plugin keeps in sync.
/// The editor reads this to display sign-in state in the title bar without
/// depending on the full `renzora_auth` crate.
#[derive(Resource, Default, Clone)]
pub struct AuthBridge {
    /// Whether the auth sign-in window is currently open.
    pub window_open: bool,
    /// The signed-in username, if any.
    pub signed_in_username: Option<String>,
}

/// Marker resource inserted for one frame when sign-in succeeds.
/// The editor can consume this to react (e.g. switch to the Hub layout).
#[derive(Resource)]
pub struct AuthJustSignedIn;

/// Event-like resource: requests the auth window to toggle open/closed.
#[derive(Resource)]
pub struct AuthToggleWindowRequest;

/// Event-like resource: requests sign-out.
#[derive(Resource)]
pub struct AuthSignOutRequest;

// ============================================================================
// PropertyValue (shared between scripting and blueprints)
// ============================================================================

/// Value types for property writes and reflection-based get/set.
#[derive(Clone, Debug)]
pub enum PropertyValue {
    Float(f32),
    Int(i64),
    Bool(bool),
    String(String),
    Vec3([f32; 3]),
    Color([f32; 4]),
}

// ============================================================================
// ScriptInput (shared between scripting and blueprints)
// ============================================================================

/// Input state resource collected each frame for scripts and blueprints.
#[derive(Resource, Default, Clone)]
pub struct ScriptInput {
    pub keys_pressed: HashMap<KeyCode, bool>,
    pub keys_just_pressed: HashMap<KeyCode, bool>,
    pub keys_just_released: HashMap<KeyCode, bool>,
    pub mouse_pressed: HashMap<MouseButton, bool>,
    pub mouse_just_pressed: HashMap<MouseButton, bool>,
    pub mouse_position: Vec2,
    pub mouse_delta: Vec2,
    pub scroll_delta: Vec2,
    pub gamepad_axes: HashMap<u32, HashMap<GamepadAxis, f32>>,
    pub gamepad_buttons: HashMap<u32, HashMap<GamepadButton, bool>>,
}

impl ScriptInput {
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.keys_pressed.get(&key).copied().unwrap_or(false)
    }

    pub fn is_key_just_pressed(&self, key: KeyCode) -> bool {
        self.keys_just_pressed.get(&key).copied().unwrap_or(false)
    }

    pub fn get_movement_vector(&self) -> Vec2 {
        let mut x = 0.0f32;
        let mut y = 0.0f32;
        if self.is_key_pressed(KeyCode::KeyA) || self.is_key_pressed(KeyCode::ArrowLeft) { x -= 1.0; }
        if self.is_key_pressed(KeyCode::KeyD) || self.is_key_pressed(KeyCode::ArrowRight) { x += 1.0; }
        if self.is_key_pressed(KeyCode::KeyS) || self.is_key_pressed(KeyCode::ArrowDown) { y -= 1.0; }
        if self.is_key_pressed(KeyCode::KeyW) || self.is_key_pressed(KeyCode::ArrowUp) { y += 1.0; }
        let v = Vec2::new(x, y);
        if v.length_squared() > 0.0 { v.normalize() } else { v }
    }

    pub fn get_gamepad_left_stick(&self, id: u32) -> Vec2 {
        let axes = match self.gamepad_axes.get(&id) { Some(a) => a, None => return Vec2::ZERO };
        Vec2::new(
            axes.get(&GamepadAxis::LeftStickX).copied().unwrap_or(0.0),
            axes.get(&GamepadAxis::LeftStickY).copied().unwrap_or(0.0),
        )
    }

    pub fn get_gamepad_right_stick(&self, id: u32) -> Vec2 {
        let axes = match self.gamepad_axes.get(&id) { Some(a) => a, None => return Vec2::ZERO };
        Vec2::new(
            axes.get(&GamepadAxis::RightStickX).copied().unwrap_or(0.0),
            axes.get(&GamepadAxis::RightStickY).copied().unwrap_or(0.0),
        )
    }

    pub fn get_gamepad_trigger(&self, id: u32, left: bool) -> f32 {
        let axes = match self.gamepad_axes.get(&id) { Some(a) => a, None => return 0.0 };
        let axis = if left { GamepadAxis::LeftZ } else { GamepadAxis::RightZ };
        axes.get(&axis).copied().unwrap_or(0.0)
    }

    pub fn is_gamepad_button_pressed(&self, id: u32, button: GamepadButton) -> bool {
        self.gamepad_buttons.get(&id)
            .and_then(|b| b.get(&button))
            .copied()
            .unwrap_or(false)
    }
}

// ============================================================================
// Graph types (shared between blueprint and editor crates)
// ============================================================================

/// Node identifier in a visual scripting graph.
pub type NodeId = u64;

/// Pin data types for blueprint nodes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, Reflect)]
pub enum PinType {
    /// Execution flow (white wires) — controls order of operations.
    Exec,
    Float,
    Int,
    Bool,
    String,
    Vec2,
    Vec3,
    Color,
    Entity,
    /// Wildcard — accepts any data type.
    Any,
}

impl PinType {
    /// Can `from` connect to `to`?
    pub fn compatible(from: PinType, to: PinType) -> bool {
        if from == to {
            return true;
        }
        if to == PinType::Any && from != PinType::Exec {
            return true;
        }
        if from == PinType::Any && to != PinType::Exec {
            return true;
        }
        matches!(
            (from, to),
            (PinType::Int, PinType::Float)
            | (PinType::Float, PinType::Vec2 | PinType::Vec3 | PinType::Color)
            | (PinType::Vec3, PinType::Color)
            | (PinType::Color, PinType::Vec3)
            | (PinType::Bool, PinType::Int | PinType::Float)
        )
    }
}

/// Pin direction.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, Reflect)]
pub enum PinDir {
    Input,
    Output,
}

/// Concrete values stored on pins (inline constants, defaults).
#[derive(Clone, Debug, Serialize, Deserialize, Reflect)]
pub enum PinValue {
    None,
    Float(f32),
    Int(i32),
    Bool(bool),
    String(String),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Color([f32; 4]),
    Entity(String),
}

impl Default for PinValue {
    fn default() -> Self {
        Self::None
    }
}

impl PinValue {
    pub fn as_float(&self) -> f32 {
        match self {
            Self::Float(v) => *v,
            Self::Int(v) => *v as f32,
            Self::Bool(v) => if *v { 1.0 } else { 0.0 },
            _ => 0.0,
        }
    }

    pub fn as_int(&self) -> i32 {
        match self {
            Self::Int(v) => *v,
            Self::Float(v) => *v as i32,
            Self::Bool(v) => if *v { 1 } else { 0 },
            _ => 0,
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            Self::Bool(v) => *v,
            Self::Float(v) => *v != 0.0,
            Self::Int(v) => *v != 0,
            _ => false,
        }
    }

    pub fn as_string(&self) -> String {
        match self {
            Self::String(v) => v.clone(),
            Self::Float(v) => format!("{v}"),
            Self::Int(v) => format!("{v}"),
            Self::Bool(v) => format!("{v}"),
            Self::Entity(v) => v.clone(),
            _ => String::new(),
        }
    }

    pub fn as_vec3(&self) -> [f32; 3] {
        match self {
            Self::Vec3(v) => *v,
            Self::Color([r, g, b, _]) => [*r, *g, *b],
            Self::Float(v) => [*v, *v, *v],
            _ => [0.0, 0.0, 0.0],
        }
    }

    pub fn as_vec2(&self) -> [f32; 2] {
        match self {
            Self::Vec2(v) => *v,
            Self::Float(v) => [*v, *v],
            _ => [0.0, 0.0],
        }
    }

    pub fn as_color(&self) -> [f32; 4] {
        match self {
            Self::Color(v) => *v,
            Self::Vec3([r, g, b]) => [*r, *g, *b, 1.0],
            Self::Float(v) => [*v, *v, *v, 1.0],
            _ => [1.0, 1.0, 1.0, 1.0],
        }
    }

    pub fn pin_type(&self) -> PinType {
        match self {
            Self::None => PinType::Any,
            Self::Float(_) => PinType::Float,
            Self::Int(_) => PinType::Int,
            Self::Bool(_) => PinType::Bool,
            Self::String(_) => PinType::String,
            Self::Vec2(_) => PinType::Vec2,
            Self::Vec3(_) => PinType::Vec3,
            Self::Color(_) => PinType::Color,
            Self::Entity(_) => PinType::Entity,
        }
    }
}

/// Describes a pin on a node type (static definition).
#[derive(Clone, Debug)]
pub struct PinTemplate {
    pub name: String,
    pub label: String,
    pub pin_type: PinType,
    pub direction: PinDir,
    pub default_value: PinValue,
}

impl PinTemplate {
    pub fn exec_in(name: &str, label: &str) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            pin_type: PinType::Exec,
            direction: PinDir::Input,
            default_value: PinValue::None,
        }
    }

    pub fn exec_out(name: &str, label: &str) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            pin_type: PinType::Exec,
            direction: PinDir::Output,
            default_value: PinValue::None,
        }
    }

    pub fn input(name: &str, label: &str, pin_type: PinType) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            pin_type,
            direction: PinDir::Input,
            default_value: PinValue::None,
        }
    }

    pub fn output(name: &str, label: &str, pin_type: PinType) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            pin_type,
            direction: PinDir::Output,
            default_value: PinValue::None,
        }
    }

    pub fn with_default(mut self, value: PinValue) -> Self {
        self.default_value = value;
        self
    }
}

/// Static definition of a blueprint node type.
pub struct BlueprintNodeDef {
    pub node_type: &'static str,
    pub display_name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub pins: fn() -> Vec<PinTemplate>,
    /// RGB header color for the node in the graph editor.
    pub color: [u8; 3],
}

/// A connection between two pins in a graph.
#[derive(Clone, Debug, Serialize, Deserialize, Reflect)]
pub struct BlueprintConnection {
    pub from_node: NodeId,
    pub from_pin: String,
    pub to_node: NodeId,
    pub to_pin: String,
}

/// A node instance in a graph.
#[derive(Clone, Debug, Serialize, Deserialize, Reflect)]
pub struct BlueprintNode {
    pub id: NodeId,
    pub node_type: String,
    pub position: [f32; 2],
    /// Override values for input pins (user-set constants).
    pub input_values: HashMap<String, PinValue>,
}

impl BlueprintNode {
    pub fn new(id: NodeId, node_type: &str, position: [f32; 2]) -> Self {
        Self {
            id,
            node_type: node_type.to_string(),
            position,
            input_values: HashMap::new(),
        }
    }

    pub fn get_input_value(&self, pin_name: &str) -> Option<&PinValue> {
        self.input_values.get(pin_name)
    }
}
