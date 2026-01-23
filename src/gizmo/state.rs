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
    /// Snapping settings
    pub snap: SnapSettings,
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
            snap: SnapSettings::default(),
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
