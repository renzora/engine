use bevy::prelude::*;

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
        }
    }
}

impl EditorSettings {
    /// Get grid color as a Color
    pub fn grid_color_as_color(&self) -> Color {
        Color::srgb(self.grid_color[0], self.grid_color[1], self.grid_color[2])
    }
}
