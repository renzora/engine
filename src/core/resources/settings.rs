use bevy::prelude::*;

/// Visualization mode for debug rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VisualizationMode {
    #[default]
    None,
    Normals,
    Roughness,
    Metallic,
    Depth,
    UvChecker,
}

impl VisualizationMode {
    pub fn label(&self) -> &'static str {
        match self {
            VisualizationMode::None => "None",
            VisualizationMode::Normals => "Normals",
            VisualizationMode::Roughness => "Roughness",
            VisualizationMode::Metallic => "Metallic",
            VisualizationMode::Depth => "Depth",
            VisualizationMode::UvChecker => "UV Checker",
        }
    }

    pub const ALL: &'static [VisualizationMode] = &[
        VisualizationMode::None,
        VisualizationMode::Normals,
        VisualizationMode::Roughness,
        VisualizationMode::Metallic,
        VisualizationMode::Depth,
        VisualizationMode::UvChecker,
    ];
}

/// Render toggles that can be combined
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderToggles {
    /// Show textures (false = solid colors only)
    pub textures: bool,
    /// Show wireframe overlay
    pub wireframe: bool,
    /// Enable lighting (false = unlit/fullbright)
    pub lighting: bool,
    /// Enable shadows
    pub shadows: bool,
}

impl Default for RenderToggles {
    fn default() -> Self {
        Self {
            textures: true,
            wireframe: false,
            lighting: true,
            shadows: true,
        }
    }
}

/// Editor settings and preferences
#[derive(Resource)]
pub struct EditorSettings {
    /// Show the settings window
    pub show_settings_window: bool,
    /// Camera movement speed
    pub camera_move_speed: f32,
    /// Whether to show the grid
    pub show_grid: bool,
    /// Size of the grid
    pub grid_size: f32,
    /// Number of grid divisions
    pub grid_divisions: u32,
    /// Color of the grid lines
    pub grid_color: [f32; 3],
    /// Show demo window (debug)
    pub show_demo_window: bool,
    /// Splash screen - new project name
    pub new_project_name: String,
    /// Render toggles (textures, wireframe, lighting, shadows)
    pub render_toggles: RenderToggles,
    /// Debug visualization mode
    pub visualization_mode: VisualizationMode,
    /// Developer mode - enables plugin development tools
    pub dev_mode: bool,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            show_settings_window: false,
            camera_move_speed: 10.0,
            show_grid: true,
            grid_size: 10.0,
            grid_divisions: 10,
            grid_color: [0.3, 0.3, 0.3],
            show_demo_window: false,
            new_project_name: String::new(),
            render_toggles: RenderToggles::default(),
            visualization_mode: VisualizationMode::default(),
            dev_mode: false,
        }
    }
}

impl EditorSettings {
    /// Get grid color as a Color
    pub fn grid_color_as_color(&self) -> Color {
        Color::srgb(self.grid_color[0], self.grid_color[1], self.grid_color[2])
    }
}
