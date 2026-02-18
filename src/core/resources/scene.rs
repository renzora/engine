#![allow(dead_code)]

use bevy::prelude::*;
use std::path::PathBuf;

use super::camera::TabCameraState;

/// Kind of document tab
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabKind {
    Scene(usize),
    Script(usize),
    Blueprint(String),
    Image(usize),
    Video(usize),
    Audio(usize),
    Animation(usize),
    Texture(usize),
    ParticleFX(usize),
    Level(usize),
    Terrain(usize),
    Shader(usize),
}

impl TabKind {
    /// Get the layout name associated with this document type
    pub fn layout_name(&self) -> &'static str {
        match self {
            TabKind::Scene(_) => "Scene",
            TabKind::Script(_) => "Scripting",
            TabKind::Blueprint(_) => "Blueprints",
            TabKind::Image(_) => "Image Preview",
            TabKind::Video(_) => "Video Editor",
            TabKind::Audio(_) => "DAW",
            TabKind::Animation(_) => "Animation",
            TabKind::Texture(_) => "Scene",
            TabKind::ParticleFX(_) => "Particles",
            TabKind::Level(_) => "Level Design",
            TabKind::Terrain(_) => "Terrain",
            TabKind::Shader(_) => "Shaders",
        }
    }
}

/// Build state for Rust plugins
#[derive(Default, Clone)]
pub enum BuildState {
    #[default]
    Idle,
    Building,
    Success(String),  // Plugin name
    Failed(Vec<BuildError>),
}

/// A build error from cargo
#[derive(Clone, Debug)]
pub struct BuildError {
    pub message: String,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

/// State for scene file management and tabs
#[derive(Resource)]
pub struct SceneManagerState {
    /// Current scene file path (for active tab)
    pub current_scene_path: Option<PathBuf>,
    /// Request to save the current scene
    pub save_scene_requested: bool,
    /// Request to save the current scene with new name
    pub save_scene_as_requested: bool,
    /// Request to create a new scene
    pub new_scene_requested: bool,
    /// Request to open a scene file
    pub open_scene_requested: bool,
    /// Request to create a new project (return to splash)
    pub new_project_requested: bool,
    /// Request to open a different project
    pub open_project_requested: bool,
    /// Open scene tabs
    pub scene_tabs: Vec<SceneTab>,
    /// Index of the active scene tab
    pub active_scene_tab: usize,
    /// Pending tab switch (set by UI, processed by scene manager)
    pub pending_tab_switch: Option<usize>,
    /// Pending tab close request
    pub pending_tab_close: Option<usize>,
    /// Open scripts in the script editor
    pub open_scripts: Vec<OpenScript>,
    /// Active script tab index
    pub active_script_tab: Option<usize>,
    /// Open images in the image preview panel
    pub open_images: Vec<OpenImage>,
    /// Active image tab index
    pub active_image_tab: Option<usize>,
    /// Open video projects
    pub open_videos: Vec<OpenVideo>,
    /// Active video tab index
    pub active_video_tab: Option<usize>,
    /// Open audio projects
    pub open_audios: Vec<OpenAudio>,
    /// Active audio tab index
    pub active_audio_tab: Option<usize>,
    /// Open animation files
    pub open_animations: Vec<OpenAnimation>,
    /// Active animation tab index
    pub active_animation_tab: Option<usize>,
    /// Open texture files
    pub open_textures: Vec<OpenTexture>,
    /// Active texture tab index
    pub active_texture_tab: Option<usize>,
    /// Open particle FX files
    pub open_particles: Vec<OpenParticleFX>,
    /// Active particle tab index
    pub active_particle_tab: Option<usize>,
    /// Open level files
    pub open_levels: Vec<OpenLevel>,
    /// Active level tab index
    pub active_level_tab: Option<usize>,
    /// Open terrain files
    pub open_terrains: Vec<OpenTerrain>,
    /// Active terrain tab index
    pub active_terrain_tab: Option<usize>,
    /// The currently active document (used for tab highlighting and layout switching)
    pub active_document: Option<TabKind>,
    /// Recently saved scene paths - scene instances referencing these need to reload
    pub recently_saved_scenes: Vec<PathBuf>,
    /// Build state for Rust plugin development
    pub build_state: BuildState,
    /// Unified tab order - stores the order of all tabs (scenes and scripts together)
    pub tab_order: Vec<TabKind>,
    /// Timer for auto-save (seconds since last auto-save check)
    pub auto_save_timer: f32,
    /// Whether auto-save is enabled
    pub auto_save_enabled: bool,
    /// Auto-save interval in seconds
    pub auto_save_interval: f32,
    /// Request to export the project as a standalone game
    pub export_project_requested: bool,
}

impl Default for SceneManagerState {
    fn default() -> Self {
        Self {
            current_scene_path: None,
            save_scene_requested: false,
            save_scene_as_requested: false,
            new_scene_requested: false,
            open_scene_requested: false,
            new_project_requested: false,
            open_project_requested: false,
            scene_tabs: vec![SceneTab {
                name: "Untitled".to_string(),
                ..Default::default()
            }],
            active_scene_tab: 0,
            pending_tab_switch: None,
            pending_tab_close: None,
            open_scripts: Vec::new(),
            active_script_tab: None,
            open_images: Vec::new(),
            active_image_tab: None,
            open_videos: Vec::new(),
            active_video_tab: None,
            open_audios: Vec::new(),
            active_audio_tab: None,
            open_animations: Vec::new(),
            active_animation_tab: None,
            open_textures: Vec::new(),
            active_texture_tab: None,
            open_particles: Vec::new(),
            active_particle_tab: None,
            open_levels: Vec::new(),
            active_level_tab: None,
            open_terrains: Vec::new(),
            active_terrain_tab: None,
            active_document: Some(TabKind::Scene(0)),
            recently_saved_scenes: Vec::new(),
            build_state: BuildState::default(),
            tab_order: vec![TabKind::Scene(0)],
            auto_save_timer: 0.0,
            auto_save_enabled: true,
            auto_save_interval: 30.0, // Auto-save every 30 seconds when modified
            export_project_requested: false,
        }
    }
}

impl SceneManagerState {
    /// Get the active scene tab
    pub fn active_tab(&self) -> Option<&SceneTab> {
        self.scene_tabs.get(self.active_scene_tab)
    }

    /// Get the active scene tab mutably
    pub fn active_tab_mut(&mut self) -> Option<&mut SceneTab> {
        self.scene_tabs.get_mut(self.active_scene_tab)
    }

    /// Mark the active scene as modified
    pub fn mark_modified(&mut self) {
        if let Some(tab) = self.active_tab_mut() {
            tab.is_modified = true;
        }
    }

    /// Add a new scene tab
    pub fn add_tab(&mut self, name: String, path: Option<PathBuf>) -> usize {
        let tab = SceneTab {
            name,
            path,
            is_modified: false,
            camera_state: None,
        };
        self.scene_tabs.push(tab);
        self.scene_tabs.len() - 1
    }

    /// Request to switch to a specific tab
    pub fn switch_to_tab(&mut self, index: usize) {
        if index < self.scene_tabs.len() {
            self.pending_tab_switch = Some(index);
        }
    }

    /// Request to close a specific tab
    pub fn close_tab(&mut self, index: usize) {
        if index < self.scene_tabs.len() {
            self.pending_tab_close = Some(index);
        }
    }

    /// Set the active document and update all related state
    pub fn set_active_document(&mut self, tab_kind: TabKind) {
        // Clear all active tab states
        self.active_script_tab = None;
        self.active_image_tab = None;
        self.active_video_tab = None;
        self.active_audio_tab = None;
        self.active_animation_tab = None;
        self.active_texture_tab = None;
        self.active_particle_tab = None;
        self.active_level_tab = None;
        self.active_terrain_tab = None;

        // Set the specific active tab based on type
        match &tab_kind {
            TabKind::Scene(idx) => {
                self.pending_tab_switch = Some(*idx);
            }
            TabKind::Script(idx) => {
                self.active_script_tab = Some(*idx);
            }
            TabKind::Blueprint(_) => {
                // Blueprint active state is handled by BlueprintEditorState
            }
            TabKind::Image(idx) => {
                self.active_image_tab = Some(*idx);
            }
            TabKind::Video(idx) => {
                self.active_video_tab = Some(*idx);
            }
            TabKind::Audio(idx) => {
                self.active_audio_tab = Some(*idx);
            }
            TabKind::Animation(idx) => {
                self.active_animation_tab = Some(*idx);
            }
            TabKind::Texture(idx) => {
                self.active_texture_tab = Some(*idx);
            }
            TabKind::ParticleFX(idx) => {
                self.active_particle_tab = Some(*idx);
            }
            TabKind::Level(idx) => {
                self.active_level_tab = Some(*idx);
            }
            TabKind::Terrain(idx) => {
                self.active_terrain_tab = Some(*idx);
            }
            TabKind::Shader(idx) => {
                self.active_script_tab = Some(*idx);
            }
        }

        // Set the unified active document
        self.active_document = Some(tab_kind);
    }

    /// Check if a specific tab is the active document
    pub fn is_active_document(&self, tab_kind: &TabKind) -> bool {
        self.active_document.as_ref() == Some(tab_kind)
    }
}

/// Represents an open scene tab
#[derive(Clone, Debug, Default)]
pub struct SceneTab {
    pub name: String,
    pub path: Option<PathBuf>,
    pub is_modified: bool,
    /// Stored camera state when switching away from tab
    pub camera_state: Option<TabCameraState>,
}

/// Represents an open script in the editor
#[derive(Clone, Debug)]
pub struct OpenScript {
    pub path: PathBuf,
    pub name: String,
    pub content: String,
    pub is_modified: bool,
    /// Compilation error message (if any)
    pub error: Option<ScriptError>,
    /// Last content that was checked for errors
    pub last_checked_content: String,
}

/// Script compilation error information
#[derive(Clone, Debug)]
pub struct ScriptError {
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

/// Represents an open image in the preview panel
#[derive(Clone, Debug)]
pub struct OpenImage {
    pub path: PathBuf,
    pub name: String,
    /// Zoom level (1.0 = 100%)
    pub zoom: f32,
    /// Pan offset for viewing
    pub pan_offset: (f32, f32),
}

/// Represents an open video project
#[derive(Clone, Debug)]
pub struct OpenVideo {
    pub path: PathBuf,
    pub name: String,
    pub is_modified: bool,
}

/// Represents an open audio project (DAW)
#[derive(Clone, Debug)]
pub struct OpenAudio {
    pub path: PathBuf,
    pub name: String,
    pub is_modified: bool,
}

/// Represents an open animation file
#[derive(Clone, Debug)]
pub struct OpenAnimation {
    pub path: PathBuf,
    pub name: String,
    pub is_modified: bool,
}

/// Represents an open texture file
#[derive(Clone, Debug)]
pub struct OpenTexture {
    pub path: PathBuf,
    pub name: String,
    pub is_modified: bool,
}

/// Represents an open particle FX file
#[derive(Clone, Debug)]
pub struct OpenParticleFX {
    pub path: PathBuf,
    pub name: String,
    pub is_modified: bool,
}

/// Represents an open level file (special scene type)
#[derive(Clone, Debug)]
pub struct OpenLevel {
    pub path: PathBuf,
    pub name: String,
    pub is_modified: bool,
}

/// Represents an open terrain file (special scene type)
#[derive(Clone, Debug)]
pub struct OpenTerrain {
    pub path: PathBuf,
    pub name: String,
    pub is_modified: bool,
}
