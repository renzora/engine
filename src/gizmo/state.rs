use bevy::prelude::*;

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

/// State for the gizmo system
#[derive(Resource)]
pub struct GizmoState {
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
}

impl Default for GizmoState {
    fn default() -> Self {
        Self {
            mode: GizmoMode::Translate,
            hovered_axis: None,
            is_dragging: false,
            drag_axis: None,
            drag_start_offset: Vec3::ZERO,
            drag_start_angle: 0.0,
            drag_start_rotation: Quat::IDENTITY,
            drag_start_scale: Vec3::ONE,
            drag_start_distance: 0.0,
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
