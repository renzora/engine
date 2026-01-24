use bevy::prelude::*;
use std::path::PathBuf;

use super::camera::TabCameraState;

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
    /// Recently saved scene paths - scene instances referencing these need to reload
    pub recently_saved_scenes: Vec<PathBuf>,
    /// Build state for Rust plugin development
    pub build_state: BuildState,
}

impl Default for SceneManagerState {
    fn default() -> Self {
        Self {
            current_scene_path: None,
            save_scene_requested: false,
            save_scene_as_requested: false,
            new_scene_requested: false,
            open_scene_requested: false,
            scene_tabs: vec![SceneTab {
                name: "Untitled".to_string(),
                ..Default::default()
            }],
            active_scene_tab: 0,
            pending_tab_switch: None,
            pending_tab_close: None,
            open_scripts: Vec::new(),
            active_script_tab: None,
            recently_saved_scenes: Vec::new(),
            build_state: BuildState::default(),
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
