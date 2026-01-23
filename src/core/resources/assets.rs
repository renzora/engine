use bevy::prelude::*;
use std::path::PathBuf;

/// State for the asset browser panel
#[derive(Resource, Default)]
pub struct AssetBrowserState {
    /// Current folder being viewed in assets panel
    pub current_folder: Option<PathBuf>,
    /// Currently selected asset
    pub selected_asset: Option<PathBuf>,
    /// Asset being dragged
    pub dragging_asset: Option<PathBuf>,
    /// Pending asset drop (path, 3D position) - for viewport drops
    pub pending_asset_drop: Option<(PathBuf, Vec3)>,
    /// Pending scene drop to hierarchy (scene path, parent entity)
    pub pending_scene_drop: Option<(PathBuf, Option<Entity>)>,
    /// Search filter text
    pub search: String,
    /// Current view mode (grid or list)
    pub view_mode: AssetViewMode,
    /// Zoom level for grid view
    pub zoom: f32,
    /// Show create script dialog
    pub show_create_script_dialog: bool,
    /// New script name being entered
    pub new_script_name: String,
    /// Request to import an asset
    pub import_asset_requested: bool,
    /// Show create folder dialog
    pub show_create_folder_dialog: bool,
    /// New folder name being entered
    pub new_folder_name: String,
}

impl AssetBrowserState {
    pub fn new() -> Self {
        Self {
            zoom: 1.0,
            ..Default::default()
        }
    }

    /// Navigate to a folder
    pub fn navigate_to(&mut self, path: PathBuf) {
        self.current_folder = Some(path);
        self.selected_asset = None;
    }

    /// Navigate up one folder level
    pub fn navigate_up(&mut self) {
        if let Some(current) = &self.current_folder {
            if let Some(parent) = current.parent() {
                self.current_folder = Some(parent.to_path_buf());
                self.selected_asset = None;
            }
        }
    }

    /// Select an asset
    pub fn select(&mut self, path: PathBuf) {
        self.selected_asset = Some(path);
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        self.selected_asset = None;
    }

    /// Start dragging an asset
    pub fn start_drag(&mut self, path: PathBuf) {
        self.dragging_asset = Some(path);
    }

    /// End dragging
    pub fn end_drag(&mut self) {
        self.dragging_asset = None;
    }

    /// Check if search matches a filename
    pub fn matches_search(&self, filename: &str) -> bool {
        if self.search.is_empty() {
            return true;
        }
        filename.to_lowercase().contains(&self.search.to_lowercase())
    }
}

/// View mode for asset browser
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum AssetViewMode {
    #[default]
    Grid,
    List,
}
