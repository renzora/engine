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

/// Collision gizmo visibility mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CollisionGizmoVisibility {
    /// Only show collision gizmos for selected entities
    #[default]
    SelectedOnly,
    /// Always show all collision gizmos
    Always,
}

/// Currently selected settings tab
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    General,
    Viewport,
    Shortcuts,
    Theme,
    Updates,
}

/// Camera sensitivity settings
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraSettings {
    /// Movement speed (fly mode)
    pub move_speed: f32,
    /// Look/rotation sensitivity
    pub look_sensitivity: f32,
    /// Orbit sensitivity
    pub orbit_sensitivity: f32,
    /// Pan sensitivity
    pub pan_sensitivity: f32,
    /// Zoom sensitivity (scroll wheel)
    pub zoom_sensitivity: f32,
    /// Invert Y axis for look/orbit
    pub invert_y: bool,
    /// Scale movement speed based on camera distance from focus
    pub distance_relative_speed: bool,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            move_speed: 10.0,
            look_sensitivity: 0.3,
            orbit_sensitivity: 0.5,
            pan_sensitivity: 1.0,
            zoom_sensitivity: 1.0,
            invert_y: false,
            distance_relative_speed: true,
        }
    }
}

/// Editor settings and preferences
#[derive(Resource)]
pub struct EditorSettings {
    /// Currently selected settings tab
    pub settings_tab: SettingsTab,
    /// Camera movement speed (deprecated, use camera_settings.move_speed)
    pub camera_move_speed: f32,
    /// Camera settings
    pub camera_settings: CameraSettings,
    /// Whether to show the grid
    pub show_grid: bool,
    /// Size of the grid
    pub grid_size: f32,
    /// Number of grid divisions
    pub grid_divisions: u32,
    /// Color of the grid lines
    pub grid_color: [f32; 3],
    /// Collision gizmo visibility mode
    pub collision_gizmo_visibility: CollisionGizmoVisibility,
    /// Show demo window (debug)
    #[allow(dead_code)]
    pub show_demo_window: bool,
    /// Splash screen - new project name
    pub new_project_name: String,
    /// Render toggles (textures, wireframe, lighting, shadows)
    pub render_toggles: RenderToggles,
    /// Debug visualization mode
    pub visualization_mode: VisualizationMode,
    /// Developer mode - enables plugin development tools
    pub dev_mode: bool,
    /// Base font size in points
    pub font_size: f32,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            settings_tab: SettingsTab::default(),
            camera_move_speed: 10.0,
            camera_settings: CameraSettings::default(),
            show_grid: true,
            grid_size: 10.0,
            grid_divisions: 10,
            grid_color: [0.3, 0.3, 0.3],
            collision_gizmo_visibility: CollisionGizmoVisibility::default(),
            show_demo_window: false,
            new_project_name: String::new(),
            render_toggles: RenderToggles::default(),
            visualization_mode: VisualizationMode::default(),
            dev_mode: false,
            font_size: 13.0,
        }
    }
}

