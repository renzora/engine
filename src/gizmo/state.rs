#![allow(dead_code)]

use bevy::prelude::*;

/// Current editor tool mode
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum EditorTool {
    /// Select mode - click to select, drag for box selection
    #[default]
    Select,
    /// Transform mode - shows gizmo for active transform operation
    Transform,
    /// Brush mode - click-drag to create level geometry
    Brush,
    /// Block edit mode - resize brush geometry with face handles
    BlockEdit,
    /// Terrain sculpt mode - paint on terrain to modify height
    TerrainSculpt,
    /// Surface paint mode - paint material layers onto meshes
    SurfacePaint,
}

/// Current gizmo transformation mode
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum GizmoMode {
    #[default]
    Translate,
    Rotate,
    Scale,
}

/// Axis or plane being dragged
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

/// Snapping settings for transform operations
#[derive(Clone, Copy, PartialEq)]
pub struct SnapSettings {
    /// Enable position snapping
    pub translate_enabled: bool,
    /// Position snap increment (in units)
    pub translate_snap: f32,
    /// Enable rotation snapping
    pub rotate_enabled: bool,
    /// Rotation snap increment (in degrees)
    pub rotate_snap: f32,
    /// Enable scale snapping
    pub scale_enabled: bool,
    /// Scale snap increment
    pub scale_snap: f32,
    /// Enable snap to nearby objects
    pub object_snap_enabled: bool,
    /// Distance threshold for snapping to objects (in units)
    pub object_snap_distance: f32,
    /// Enable snap to floor when no objects nearby
    pub floor_snap_enabled: bool,
    /// Y position of the floor
    pub floor_y: f32,
}

impl Default for SnapSettings {
    fn default() -> Self {
        Self {
            translate_enabled: false,
            translate_snap: 1.0,
            rotate_enabled: false,
            rotate_snap: 15.0,
            scale_enabled: false,
            scale_snap: 0.25,
            object_snap_enabled: true,
            object_snap_distance: 0.5,
            floor_snap_enabled: true,
            floor_y: 0.0,
        }
    }
}

impl SnapSettings {
    /// Snap a position value if snapping is enabled
    pub fn snap_translate(&self, value: f32) -> f32 {
        if self.translate_enabled && self.translate_snap > 0.0 {
            (value / self.translate_snap).round() * self.translate_snap
        } else {
            value
        }
    }

    /// Snap a Vec3 position if snapping is enabled
    pub fn snap_translate_vec3(&self, value: Vec3) -> Vec3 {
        Vec3::new(
            self.snap_translate(value.x),
            self.snap_translate(value.y),
            self.snap_translate(value.z),
        )
    }

    /// Snap a rotation value (in radians) if snapping is enabled
    pub fn snap_rotate(&self, radians: f32) -> f32 {
        if self.rotate_enabled && self.rotate_snap > 0.0 {
            let degrees = radians.to_degrees();
            let snapped = (degrees / self.rotate_snap).round() * self.rotate_snap;
            snapped.to_radians()
        } else {
            radians
        }
    }

    /// Snap a scale value if snapping is enabled
    pub fn snap_scale(&self, value: f32) -> f32 {
        if self.scale_enabled && self.scale_snap > 0.0 {
            (value / self.scale_snap).round() * self.scale_snap
        } else {
            value
        }
    }

    /// Snap a Vec3 scale if snapping is enabled
    pub fn snap_scale_vec3(&self, value: Vec3) -> Vec3 {
        Vec3::new(
            self.snap_scale(value.x),
            self.snap_scale(value.y),
            self.snap_scale(value.z),
        )
    }
}

/// State for box selection (drag to select multiple objects)
#[derive(Default, Clone, Copy)]
pub struct BoxSelectionState {
    /// Whether box selection is currently active
    pub active: bool,
    /// Start position in screen coordinates
    pub start_pos: [f32; 2],
    /// Current position in screen coordinates
    pub current_pos: [f32; 2],
}

impl BoxSelectionState {
    /// Get the selection rectangle (min_x, min_y, max_x, max_y)
    pub fn get_rect(&self) -> (f32, f32, f32, f32) {
        let min_x = self.start_pos[0].min(self.current_pos[0]);
        let max_x = self.start_pos[0].max(self.current_pos[0]);
        let min_y = self.start_pos[1].min(self.current_pos[1]);
        let max_y = self.start_pos[1].max(self.current_pos[1]);
        (min_x, min_y, max_x, max_y)
    }

    /// Check if the box is large enough to be considered a drag (not just a click)
    pub fn is_drag(&self) -> bool {
        let dx = (self.current_pos[0] - self.start_pos[0]).abs();
        let dy = (self.current_pos[1] - self.start_pos[1]).abs();
        dx > 5.0 || dy > 5.0
    }
}

/// What the object is currently snapping to
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SnapTarget {
    /// Snapping to another entity's position
    Entity(Entity),
    /// Snapping to the floor
    Floor,
    /// No snapping active
    None,
}

impl Default for SnapTarget {
    fn default() -> Self {
        Self::None
    }
}

/// What part of a collider is being edited
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColliderEditHandle {
    /// Center handle - moves the offset
    Center,
    /// Face handles - resize the collider
    PosX, NegX,
    PosY, NegY,
    PosZ, NegZ,
}

/// State for collider edit mode
#[derive(Default, Clone)]
pub struct ColliderEditState {
    /// Entity whose collider is being edited
    pub entity: Option<Entity>,
    /// Currently hovered handle
    pub hovered_handle: Option<ColliderEditHandle>,
    /// Whether dragging a handle
    pub is_dragging: bool,
    /// Handle being dragged
    pub drag_handle: Option<ColliderEditHandle>,
    /// Starting offset when drag began
    pub drag_start_offset: Vec3,
    /// Starting size when drag began (half_extents, radius, or half_height)
    pub drag_start_size: Vec3,
}

impl ColliderEditState {
    /// Check if we're currently editing a collider
    pub fn is_active(&self) -> bool {
        self.entity.is_some()
    }

    /// Start editing a collider
    pub fn start_editing(&mut self, entity: Entity) {
        self.entity = Some(entity);
        self.hovered_handle = None;
        self.is_dragging = false;
        self.drag_handle = None;
    }

    /// Stop editing
    pub fn stop_editing(&mut self) {
        self.entity = None;
        self.hovered_handle = None;
        self.is_dragging = false;
        self.drag_handle = None;
    }
}

/// State for the gizmo system
#[derive(Resource)]
pub struct GizmoState {
    /// Current editor tool (Select or Transform)
    pub tool: EditorTool,
    /// Current gizmo mode (translate, rotate, scale)
    pub mode: GizmoMode,
    /// Currently hovered axis (for highlighting)
    pub hovered_axis: Option<DragAxis>,
    /// Whether a drag operation is in progress
    pub is_dragging: bool,
    /// The axis being dragged
    pub drag_axis: Option<DragAxis>,
    /// Starting offset for translation drag
    pub drag_start_offset: Vec3,
    /// Starting angle for rotation drag
    pub drag_start_angle: f32,
    /// Starting rotation for rotation drag
    pub drag_start_rotation: Quat,
    /// Starting scale for scale drag
    pub drag_start_scale: Vec3,
    /// Starting distance for scale drag
    pub drag_start_distance: f32,
    /// Snapping settings
    pub snap: SnapSettings,
    /// Transform before drag started (for undo support)
    pub drag_start_transform: Option<Transform>,
    /// Entity being dragged (for undo command)
    pub drag_entity: Option<Entity>,
    /// Box selection state
    pub box_selection: BoxSelectionState,
    /// Collider edit mode state
    pub collider_edit: ColliderEditState,
    /// Current snap target (for visual feedback)
    pub snap_target: SnapTarget,
    /// Position of the snap target (for drawing snap indicator)
    pub snap_target_position: Option<Vec3>,
    /// Whether a terrain entity is currently selected (shows terrain toolbar)
    pub terrain_selected: bool,
}

impl Default for GizmoState {
    fn default() -> Self {
        Self {
            tool: EditorTool::default(),
            mode: GizmoMode::Translate,
            hovered_axis: None,
            is_dragging: false,
            drag_axis: None,
            drag_start_offset: Vec3::ZERO,
            drag_start_angle: 0.0,
            drag_start_rotation: Quat::IDENTITY,
            drag_start_scale: Vec3::ONE,
            drag_start_distance: 0.0,
            snap: SnapSettings::default(),
            drag_start_transform: None,
            drag_entity: None,
            box_selection: BoxSelectionState::default(),
            collider_edit: ColliderEditState::default(),
            snap_target: SnapTarget::default(),
            snap_target_position: None,
            terrain_selected: false,
        }
    }
}

impl GizmoState {
    /// Start a drag operation
    pub fn start_drag(&mut self, axis: DragAxis) {
        self.is_dragging = true;
        self.drag_axis = Some(axis);
    }

    /// End the current drag operation
    pub fn end_drag(&mut self) {
        self.is_dragging = false;
        self.drag_axis = None;
    }

    /// Set the gizmo mode
    pub fn set_mode(&mut self, mode: GizmoMode) {
        self.mode = mode;
        self.end_drag();
    }

    /// Cycle to the next gizmo mode
    pub fn cycle_mode(&mut self) {
        self.mode = match self.mode {
            GizmoMode::Translate => GizmoMode::Rotate,
            GizmoMode::Rotate => GizmoMode::Scale,
            GizmoMode::Scale => GizmoMode::Translate,
        };
        self.end_drag();
    }
}
