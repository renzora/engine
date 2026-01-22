use bevy::prelude::*;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Resource)]
pub struct EditorState {
    pub selected_entity: Option<Entity>,
    pub show_demo_window: bool,
    /// Entities that should be expanded in the hierarchy tree
    pub expanded_entities: HashSet<Entity>,
    pub viewport_size: [f32; 2],
    pub viewport_position: [f32; 2],
    pub viewport_hovered: bool,
    // Panel sizes (managed manually for persistence)
    pub hierarchy_width: f32,
    pub inspector_width: f32,
    pub assets_height: f32,
    // Orbit camera state
    pub orbit_focus: Vec3,
    pub orbit_distance: f32,
    pub orbit_yaw: f32,
    pub orbit_pitch: f32,
    pub gizmo_mode: GizmoMode,
    // Gizmo interaction state
    pub hovered_axis: Option<DragAxis>,
    pub is_dragging: bool,
    pub drag_axis: Option<DragAxis>,
    pub drag_start_offset: Vec3,
    // For rotation dragging
    pub drag_start_angle: f32,
    pub drag_start_rotation: Quat,
    // For scale dragging
    pub drag_start_scale: Vec3,
    pub drag_start_distance: f32,
    // Splash screen state
    pub new_project_name: String,
    // Asset dragging
    pub dragging_asset: Option<PathBuf>,
    // Pending asset drop (path, 3D position)
    pub pending_asset_drop: Option<(PathBuf, Vec3)>,
    // Current folder being viewed in assets panel
    pub current_assets_folder: Option<PathBuf>,
    // Currently selected asset
    pub selected_asset: Option<PathBuf>,
    // Entity for context menu (right-click)
    pub context_menu_entity: Option<Entity>,
    // Hierarchy drag and drop
    pub hierarchy_drag_entity: Option<Entity>,
    pub hierarchy_drop_target: Option<HierarchyDropTarget>,
    // Window state for custom title bar
    pub window_is_maximized: bool,
    pub window_request_close: bool,
    pub window_request_minimize: bool,
    pub window_request_toggle_maximize: bool,
    pub window_start_drag: bool,
    // Manual window dragging (fallback when native drag doesn't work)
    pub window_is_being_dragged: bool,
    pub window_drag_offset: Option<(f32, f32)>,
    // Scene file management
    pub current_scene_path: Option<PathBuf>,
    pub save_scene_requested: bool,
    pub save_scene_as_requested: bool,
    pub new_scene_requested: bool,
    pub open_scene_requested: bool,
    // Scene tabs
    pub scene_tabs: Vec<SceneTab>,
    pub active_scene_tab: usize,
    pub pending_tab_switch: Option<usize>,
    pub pending_tab_close: Option<usize>,
    // Assets panel context menu
    pub show_create_script_dialog: bool,
    pub new_script_name: String,
    pub import_asset_requested: bool,
    pub show_create_folder_dialog: bool,
    pub new_folder_name: String,
    // Assets panel view settings
    pub assets_search: String,
    pub assets_view_mode: AssetViewMode,
    pub assets_zoom: f32,
    // Script editor
    pub open_scripts: Vec<OpenScript>,
    pub active_script_tab: Option<usize>,

    // Settings
    pub show_settings_window: bool,
    pub camera_move_speed: f32,
    pub show_grid: bool,
    pub grid_size: f32,
    pub grid_divisions: u32,
    pub grid_color: [f32; 3],
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

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum AssetViewMode {
    #[default]
    Grid,
    List,
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

#[derive(Clone, Debug)]
pub struct TabCameraState {
    pub orbit_focus: Vec3,
    pub orbit_distance: f32,
    pub orbit_yaw: f32,
    pub orbit_pitch: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum GizmoMode {
    #[default]
    Translate,
    Rotate,
    Scale,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DragAxis {
    X,
    Y,
    Z,
    XY,
    XZ,
    YZ,
    Free,
}

/// Where to drop a dragged hierarchy node
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HierarchyDropPosition {
    /// Insert before this entity (as sibling)
    Before,
    /// Insert after this entity (as sibling)
    After,
    /// Insert as child of this entity
    AsChild,
}

/// Drop target for hierarchy drag and drop
#[derive(Clone, Copy, Debug)]
pub struct HierarchyDropTarget {
    pub entity: Entity,
    pub position: HierarchyDropPosition,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            selected_entity: None,
            show_demo_window: false,
            expanded_entities: HashSet::new(),
            viewport_size: [1050.0, 881.0],
            viewport_position: [260.0, 56.0],
            viewport_hovered: false,
            hierarchy_width: 260.0,
            inspector_width: 320.0,
            assets_height: 200.0,
            orbit_focus: Vec3::ZERO,
            orbit_distance: 10.0,
            orbit_yaw: 0.3,
            orbit_pitch: 0.4,
            gizmo_mode: GizmoMode::Translate,
            hovered_axis: None,
            is_dragging: false,
            drag_axis: None,
            drag_start_offset: Vec3::ZERO,
            drag_start_angle: 0.0,
            drag_start_rotation: Quat::IDENTITY,
            drag_start_scale: Vec3::ONE,
            drag_start_distance: 0.0,
            new_project_name: String::new(),
            dragging_asset: None,
            pending_asset_drop: None,
            current_assets_folder: None,
            selected_asset: None,
            context_menu_entity: None,
            hierarchy_drag_entity: None,
            hierarchy_drop_target: None,
            window_is_maximized: false,
            window_request_close: false,
            window_request_minimize: false,
            window_request_toggle_maximize: false,
            window_start_drag: false,
            window_is_being_dragged: false,
            window_drag_offset: None,
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
            show_create_script_dialog: false,
            new_script_name: String::new(),
            import_asset_requested: false,
            show_create_folder_dialog: false,
            new_folder_name: String::new(),
            assets_search: String::new(),
            assets_view_mode: AssetViewMode::Grid,
            assets_zoom: 1.0,
            open_scripts: Vec::new(),
            active_script_tab: None,

            // Settings defaults
            show_settings_window: false,
            camera_move_speed: 10.0,
            show_grid: true,
            grid_size: 10.0,
            grid_divisions: 10,
            grid_color: [0.3, 0.3, 0.3],
        }
    }
}
