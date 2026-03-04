#![allow(dead_code)]

use bevy::prelude::*;

/// Current editor tool mode
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum EditorTool {
    Select,
    #[default]
    Transform,
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
    pub translate_enabled: bool,
    pub translate_snap: f32,
    pub rotate_enabled: bool,
    pub rotate_snap: f32,
    pub scale_enabled: bool,
    pub scale_snap: f32,
    pub object_snap_enabled: bool,
    pub object_snap_distance: f32,
    pub floor_snap_enabled: bool,
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
    pub fn snap_translate(&self, value: f32) -> f32 {
        if self.translate_enabled && self.translate_snap > 0.0 {
            (value / self.translate_snap).round() * self.translate_snap
        } else {
            value
        }
    }

    pub fn snap_translate_vec3(&self, value: Vec3) -> Vec3 {
        Vec3::new(
            self.snap_translate(value.x),
            self.snap_translate(value.y),
            self.snap_translate(value.z),
        )
    }

    pub fn snap_rotate(&self, radians: f32) -> f32 {
        if self.rotate_enabled && self.rotate_snap > 0.0 {
            let degrees = radians.to_degrees();
            let snapped = (degrees / self.rotate_snap).round() * self.rotate_snap;
            snapped.to_radians()
        } else {
            radians
        }
    }

    pub fn snap_scale(&self, value: f32) -> f32 {
        if self.scale_enabled && self.scale_snap > 0.0 {
            (value / self.scale_snap).round() * self.scale_snap
        } else {
            value
        }
    }

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
    pub active: bool,
    pub start_pos: [f32; 2],
    pub current_pos: [f32; 2],
}

impl BoxSelectionState {
    pub fn get_rect(&self) -> (f32, f32, f32, f32) {
        let min_x = self.start_pos[0].min(self.current_pos[0]);
        let max_x = self.start_pos[0].max(self.current_pos[0]);
        let min_y = self.start_pos[1].min(self.current_pos[1]);
        let max_y = self.start_pos[1].max(self.current_pos[1]);
        (min_x, min_y, max_x, max_y)
    }

    pub fn is_drag(&self) -> bool {
        let dx = (self.current_pos[0] - self.start_pos[0]).abs();
        let dy = (self.current_pos[1] - self.start_pos[1]).abs();
        dx > 5.0 || dy > 5.0
    }
}

/// What the object is currently snapping to
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum SnapTarget {
    Entity(Entity),
    Floor,
    #[default]
    None,
}

/// State for the gizmo system
#[derive(Resource)]
pub struct GizmoState {
    pub tool: EditorTool,
    pub mode: GizmoMode,
    pub hovered_axis: Option<DragAxis>,
    pub is_dragging: bool,
    pub drag_axis: Option<DragAxis>,
    pub drag_start_offset: Vec3,
    pub drag_start_angle: f32,
    pub drag_start_rotation: Quat,
    pub drag_start_scale: Vec3,
    pub drag_start_distance: f32,
    pub snap: SnapSettings,
    pub drag_start_transform: Option<Transform>,
    pub drag_entity: Option<Entity>,
    pub box_selection: BoxSelectionState,
    pub snap_target: SnapTarget,
    pub snap_target_position: Option<Vec3>,
    pub gizmo_scale: f32,
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
            snap_target: SnapTarget::default(),
            snap_target_position: None,
            gizmo_scale: 1.0,
        }
    }
}

impl GizmoState {
    pub fn start_drag(&mut self, axis: DragAxis) {
        self.is_dragging = true;
        self.drag_axis = Some(axis);
    }

    pub fn end_drag(&mut self) {
        self.is_dragging = false;
        self.drag_axis = None;
    }

    pub fn set_mode(&mut self, mode: GizmoMode) {
        self.mode = mode;
        self.end_drag();
    }

    pub fn cycle_mode(&mut self) {
        self.mode = match self.mode {
            GizmoMode::Translate => GizmoMode::Rotate,
            GizmoMode::Rotate => GizmoMode::Scale,
            GizmoMode::Scale => GizmoMode::Translate,
        };
        self.end_drag();
    }
}
