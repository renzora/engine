use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::ui::docking::DockingLayoutConfig;

/// Editor state configuration saved per-project
/// Stored in .editor/state.toml within the project directory
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EditorStateConfig {
    /// Layout settings
    #[serde(default)]
    pub layout: LayoutConfig,

    /// Editor settings
    #[serde(default)]
    pub settings: SettingsConfig,

    /// Asset browser settings
    #[serde(default)]
    pub asset_browser: AssetBrowserConfig,

    /// Viewport settings
    #[serde(default)]
    pub viewport: ViewportConfig,

    /// Docking/window layout settings
    #[serde(default)]
    pub docking: DockingLayoutConfig,

    /// Active theme name
    #[serde(default = "default_theme_name")]
    pub active_theme: String,
}

fn default_theme_name() -> String {
    "Dark".to_string()
}

impl Default for EditorStateConfig {
    fn default() -> Self {
        Self {
            layout: LayoutConfig::default(),
            settings: SettingsConfig::default(),
            asset_browser: AssetBrowserConfig::default(),
            viewport: ViewportConfig::default(),
            docking: DockingLayoutConfig::default(),
            active_theme: default_theme_name(),
        }
    }
}

/// Panel layout configuration
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LayoutConfig {
    /// Width of the hierarchy panel (left)
    pub hierarchy_width: f32,
    /// Width of the inspector panel (right)
    pub inspector_width: f32,
    /// Height of the assets/console panel (bottom)
    pub assets_height: f32,
    /// Which bottom panel tab is selected ("assets" or "console")
    pub bottom_panel_tab: String,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            hierarchy_width: 260.0,
            inspector_width: 320.0,
            assets_height: 200.0,
            bottom_panel_tab: "assets".to_string(),
        }
    }
}

/// General editor settings
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SettingsConfig {
    /// Developer mode enabled
    pub dev_mode: bool,
    /// Camera movement speed
    pub camera_move_speed: f32,
    /// Show grid in viewport
    pub show_grid: bool,
    /// Grid size
    pub grid_size: f32,
    /// Grid divisions
    pub grid_divisions: u32,
    /// Grid color RGB
    pub grid_color: [f32; 3],
    /// Render toggles
    #[serde(default)]
    pub render: RenderConfig,
    /// Auto-save enabled
    #[serde(default = "default_auto_save_enabled")]
    pub auto_save_enabled: bool,
    /// Auto-save interval in seconds
    #[serde(default = "default_auto_save_interval")]
    pub auto_save_interval: f32,
    /// Selection highlight mode: "outline" or "gizmo"
    #[serde(default = "default_selection_highlight_mode")]
    pub selection_highlight_mode: String,
    /// Use game camera when running scripts (ScriptsOnly mode)
    #[serde(default = "default_scripts_use_game_camera")]
    pub scripts_use_game_camera: bool,
}

fn default_selection_highlight_mode() -> String {
    "outline".to_string()
}

fn default_scripts_use_game_camera() -> bool {
    true
}

fn default_auto_save_enabled() -> bool {
    true
}

fn default_auto_save_interval() -> f32 {
    30.0
}

impl Default for SettingsConfig {
    fn default() -> Self {
        Self {
            dev_mode: false,
            camera_move_speed: 10.0,
            show_grid: true,
            grid_size: 10.0,
            grid_divisions: 10,
            grid_color: [0.3, 0.3, 0.3],
            render: RenderConfig::default(),
            auto_save_enabled: default_auto_save_enabled(),
            auto_save_interval: default_auto_save_interval(),
            selection_highlight_mode: default_selection_highlight_mode(),
            scripts_use_game_camera: default_scripts_use_game_camera(),
        }
    }
}

/// Render toggle settings
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RenderConfig {
    pub textures: bool,
    pub wireframe: bool,
    pub lighting: bool,
    pub shadows: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            textures: true,
            wireframe: false,
            lighting: true,
            shadows: true,
        }
    }
}

/// Asset browser settings
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AssetBrowserConfig {
    /// Zoom level for grid view (0.5 - 2.0)
    pub zoom: f32,
    /// View mode ("grid" or "list")
    pub view_mode: String,
}

impl Default for AssetBrowserConfig {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            view_mode: "grid".to_string(),
        }
    }
}

/// Viewport settings
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ViewportConfig {
    /// Viewport mode ("3d" or "2d")
    pub mode: String,
}

impl Default for ViewportConfig {
    fn default() -> Self {
        Self {
            mode: "3d".to_string(),
        }
    }
}

impl EditorStateConfig {
    /// Get the path to the editor state file for a project
    pub fn state_path(project_path: &Path) -> std::path::PathBuf {
        project_path.join(".editor").join("state.toml")
    }

    /// Load editor state from a project directory
    pub fn load(project_path: &Path) -> Self {
        let path = Self::state_path(project_path);
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|content| toml::from_str(&content).ok())
            .unwrap_or_default()
    }

    /// Save editor state to a project directory
    pub fn save(&self, project_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::state_path(project_path);

        // Ensure .editor directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}

/// Resource to track if editor state needs saving
#[derive(Resource, Default)]
pub struct EditorStateDirty(pub bool);

/// Resource holding the loaded editor state config (for saving back)
#[derive(Resource, Default)]
pub struct LoadedEditorState(pub Option<EditorStateConfig>);
