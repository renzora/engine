//! Blender-style modal transform system.
//!
//! Press G (grab), R (rotate), or S (scale) with an entity selected to enter modal transform mode.
//! Mouse movement applies the transform in real-time.
//! Press X/Y/Z to constrain to an axis, Shift+X/Y/Z for plane constraint.
//! Type numbers for precise values.
//! Enter/Left-click confirms, Escape/Right-click cancels.

#![allow(dead_code)]

use bevy::prelude::*;
use bevy::window::CursorOptions;

use renzora_editor::EditorSelection;
use renzora_viewport::ViewportState;
use renzora_camera::OrbitCameraState;
use renzora_keybindings::{KeyBindings, EditorAction};
use renzora_runtime::EditorCamera;

/// Modal transform mode
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModalTransformMode {
    Grab,
    Rotate,
    Scale,
}

impl ModalTransformMode {
    pub fn display_name(&self) -> &'static str {
        match self {
            ModalTransformMode::Grab => "Grab",
            ModalTransformMode::Rotate => "Rotate",
            ModalTransformMode::Scale => "Scale",
        }
    }
}

/// Axis constraint for modal transforms
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum AxisConstraint {
    #[default]
    None,
    X,
    Y,
    Z,
    PlaneYZ,
    PlaneXZ,
    PlaneXY,
}

impl AxisConstraint {
    pub fn display_name(&self) -> &'static str {
        match self {
            AxisConstraint::None => "",
            AxisConstraint::X => "X",
            AxisConstraint::Y => "Y",
            AxisConstraint::Z => "Z",
            AxisConstraint::PlaneYZ => "YZ",
            AxisConstraint::PlaneXZ => "XZ",
            AxisConstraint::PlaneXY => "XY",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            AxisConstraint::None => Color::WHITE,
            AxisConstraint::X | AxisConstraint::PlaneYZ => Color::srgb(0.93, 0.3, 0.36),
            AxisConstraint::Y | AxisConstraint::PlaneXZ => Color::srgb(0.55, 0.79, 0.25),
            AxisConstraint::Z | AxisConstraint::PlaneXY => Color::srgb(0.27, 0.54, 1.0),
        }
    }
}

/// Numeric input buffer for precise value entry
#[derive(Clone, Default, Debug)]
pub struct NumericInputBuffer {
    pub buffer: String,
    pub negative: bool,
    pub has_decimal: bool,
}

impl NumericInputBuffer {
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.negative = false;
        self.has_decimal = false;
    }

    pub fn push_digit(&mut self, digit: char) {
        if digit.is_ascii_digit() {
            self.buffer.push(digit);
        }
    }

    pub fn push_decimal(&mut self) {
        if !self.has_decimal {
            if self.buffer.is_empty() {
                self.buffer.push('0');
            }
            self.buffer.push('.');
            self.has_decimal = true;
        }
    }

    pub fn toggle_negative(&mut self) {
        self.negative = !self.negative;
    }

    pub fn backspace(&mut self) {
        if let Some(c) = self.buffer.pop() {
            if c == '.' {
                self.has_decimal = false;
            }
        } else if self.negative {
            self.negative = false;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn value(&self) -> Option<f32> {
        if self.buffer.is_empty() {
            return None;
        }
        self.buffer.parse::<f32>().ok().map(|v| {
            if self.negative { -v } else { v }
        })
    }

    pub fn display(&self) -> String {
        let sign = if self.negative { "-" } else { "" };
        if self.buffer.is_empty() {
            return sign.to_string();
        }
        format!("{}{}", sign, self.buffer)
    }
}

/// Stored transform state for an entity at the start of modal transform
#[derive(Clone)]
pub struct EntityStartState {
    pub entity: Entity,
    pub transform: Transform,
}

/// State for the modal transform system
#[derive(Resource, Default)]
pub struct ModalTransformState {
    pub active: bool,
    pub mode: Option<ModalTransformMode>,
    pub axis_constraint: AxisConstraint,
    pub numeric_input: NumericInputBuffer,
    pub accumulated_delta: Vec2,
    pub last_cursor_pos: Vec2,
    pub start_transforms: Vec<EntityStartState>,
    pub sensitivity: f32,
    pub just_warped: bool,
    pub pending_grab: bool,
    pub pivot_screen_pos: Option<Vec2>,
    pub initial_cursor_pos: Vec2,
}

impl ModalTransformState {
    pub fn start(&mut self, mode: ModalTransformMode, cursor_pos: Vec2, entities: Vec<EntityStartState>) {
        self.active = true;
        self.mode = Some(mode);
        self.axis_constraint = AxisConstraint::None;
        self.numeric_input.clear();
        self.accumulated_delta = Vec2::ZERO;
        self.last_cursor_pos = cursor_pos;
        self.initial_cursor_pos = cursor_pos;
        self.start_transforms = entities;
        self.sensitivity = 0.01;
        self.just_warped = false;
    }

    pub fn cancel(&mut self) {
        self.active = false;
        self.mode = None;
    }

    pub fn confirm(&mut self) {
        self.active = false;
        self.mode = None;
        self.start_transforms.clear();
    }

    pub fn set_axis(&mut self, axis: AxisConstraint) {
        if self.axis_constraint == axis {
            self.axis_constraint = AxisConstraint::None;
        } else {
            self.axis_constraint = axis;
        }
    }
}

/// System to detect G/R/S key press and start modal transform mode
pub fn modal_transform_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    selection: Res<EditorSelection>,
    viewport: Res<ViewportState>,
    mut modal: ResMut<ModalTransformState>,
    transforms: Query<&Transform>,
    global_transforms: Query<&GlobalTransform>,
    mut windows: Query<&mut Window>,
    mut cursor_options: Query<&mut CursorOptions>,
    camera_query: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
) {
    if modal.active {
        return;
    }

    if keybindings.rebinding.is_some() {
        return;
    }

    let Some(selected) = selection.get() else {
        return;
    };

    let selected_entities = vec![selected];

    let pending = modal.pending_grab;
    if pending {
        modal.pending_grab = false;
    }

    let mode = if pending || keybindings.just_pressed(EditorAction::ModalGrab, &keyboard) {
        Some(ModalTransformMode::Grab)
    } else if keybindings.just_pressed(EditorAction::ModalRotate, &keyboard) {
        Some(ModalTransformMode::Rotate)
    } else if keybindings.just_pressed(EditorAction::ModalScale, &keyboard) {
        Some(ModalTransformMode::Scale)
    } else {
        None
    };

    if let Some(mode) = mode {
        let Ok(mut window) = windows.single_mut() else { return };

        // Compute pivot screen position
        let mut avg_pos = Vec3::ZERO;
        let mut count = 0u32;
        for &entity in &selected_entities {
            if let Ok(gt) = global_transforms.get(entity) {
                avg_pos += gt.translation();
                count += 1;
            }
        }
        if count == 0 { return; }
        avg_pos /= count as f32;

        let pivot_screen_pos = camera_query.single().ok().and_then(|(camera, cam_transform)| {
            let ndc = camera.world_to_ndc(cam_transform, avg_pos)?;
            if ndc.z < 0.0 || ndc.z > 1.0 { return None; }
            // Use window-space coordinates
            let w = window.width();
            let h = window.height();
            Some(Vec2::new(
                (ndc.x + 1.0) * 0.5 * w,
                (1.0 - ndc.y) * 0.5 * h,
            ))
        });

        let cursor_pos = if viewport.hovered {
            window.cursor_position().unwrap_or(Vec2::ZERO)
        } else {
            let Some(pivot) = pivot_screen_pos else { return };
            window.set_cursor_position(Some(pivot));
            modal.just_warped = true;
            pivot
        };

        if !matches!(mode, ModalTransformMode::Scale) {
            if let Ok(mut cursor) = cursor_options.single_mut() {
                cursor.visible = false;
            }
        }

        let start_transforms: Vec<EntityStartState> = selected_entities
            .iter()
            .filter_map(|&entity| {
                transforms.get(entity).ok().map(|t| EntityStartState {
                    entity,
                    transform: *t,
                })
            })
            .collect();

        if !start_transforms.is_empty() {
            modal.start(mode, cursor_pos, start_transforms);
            modal.pivot_screen_pos = pivot_screen_pos;
        }
    }
}

/// System to handle keyboard input during modal transform
pub fn modal_transform_keyboard_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut modal: ResMut<ModalTransformState>,
    mut transforms: Query<&mut Transform>,
    mut cursor_options: Query<&mut CursorOptions>,
) {
    if !modal.active {
        return;
    }

    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

    // Axis constraints
    if keyboard.just_pressed(KeyCode::KeyX) {
        if shift { modal.set_axis(AxisConstraint::PlaneYZ); }
        else { modal.set_axis(AxisConstraint::X); }
    }
    if keyboard.just_pressed(KeyCode::KeyY) {
        if shift { modal.set_axis(AxisConstraint::PlaneXZ); }
        else { modal.set_axis(AxisConstraint::Y); }
    }
    if keyboard.just_pressed(KeyCode::KeyZ) {
        if shift { modal.set_axis(AxisConstraint::PlaneXY); }
        else { modal.set_axis(AxisConstraint::Z); }
    }

    // Numeric input
    let digits = [
        (KeyCode::Digit0, '0'), (KeyCode::Digit1, '1'), (KeyCode::Digit2, '2'),
        (KeyCode::Digit3, '3'), (KeyCode::Digit4, '4'), (KeyCode::Digit5, '5'),
        (KeyCode::Digit6, '6'), (KeyCode::Digit7, '7'), (KeyCode::Digit8, '8'),
        (KeyCode::Digit9, '9'),
        (KeyCode::Numpad0, '0'), (KeyCode::Numpad1, '1'), (KeyCode::Numpad2, '2'),
        (KeyCode::Numpad3, '3'), (KeyCode::Numpad4, '4'), (KeyCode::Numpad5, '5'),
        (KeyCode::Numpad6, '6'), (KeyCode::Numpad7, '7'), (KeyCode::Numpad8, '8'),
        (KeyCode::Numpad9, '9'),
    ];
    for (key, digit) in digits {
        if keyboard.just_pressed(key) {
            modal.numeric_input.push_digit(digit);
        }
    }

    if keyboard.just_pressed(KeyCode::Period) || keyboard.just_pressed(KeyCode::NumpadDecimal) {
        modal.numeric_input.push_decimal();
    }

    if keyboard.just_pressed(KeyCode::Minus) || keyboard.just_pressed(KeyCode::NumpadSubtract) {
        modal.numeric_input.toggle_negative();
    }

    if keyboard.just_pressed(KeyCode::Backspace) {
        modal.numeric_input.backspace();
    }

    // Confirm
    if keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::NumpadEnter)
        || mouse_button.just_pressed(MouseButton::Left)
    {
        modal.confirm();
        if let Ok(mut cursor) = cursor_options.single_mut() {
            cursor.visible = true;
        }
        return;
    }

    // Cancel
    if keyboard.just_pressed(KeyCode::Escape) || mouse_button.just_pressed(MouseButton::Right) {
        for state in &modal.start_transforms {
            if let Ok(mut transform) = transforms.get_mut(state.entity) {
                *transform = state.transform;
            }
        }
        modal.cancel();
        if let Ok(mut cursor) = cursor_options.single_mut() {
            cursor.visible = true;
        }
    }
}

/// System to apply transform based on mouse delta or numeric input
pub fn modal_transform_apply_system(
    mut modal: ResMut<ModalTransformState>,
    orbit: Res<OrbitCameraState>,
    _viewport: Res<ViewportState>,
    mut windows: Query<&mut Window>,
    mut transforms: Query<&mut Transform>,
) {
    if !modal.active {
        return;
    }

    let Ok(mut window) = windows.single_mut() else { return };
    let Some(current_cursor_pos) = window.cursor_position() else { return };

    if modal.just_warped {
        modal.last_cursor_pos = current_cursor_pos;
        modal.just_warped = false;
    } else {
        let frame_delta = current_cursor_pos - modal.last_cursor_pos;
        modal.accumulated_delta += frame_delta;
        modal.last_cursor_pos = current_cursor_pos;
    }

    // Cursor wrapping at viewport edges
    let margin = 5.0;
    let w = window.width();
    let h = window.height();
    let vp_min_x = margin;
    let vp_max_x = w - margin;
    let vp_min_y = margin;
    let vp_max_y = h - margin;

    let mut new_pos = current_cursor_pos;
    let mut should_warp = false;

    if current_cursor_pos.x <= vp_min_x {
        new_pos.x = vp_max_x - margin;
        should_warp = true;
    } else if current_cursor_pos.x >= vp_max_x {
        new_pos.x = vp_min_x + margin;
        should_warp = true;
    }

    if current_cursor_pos.y <= vp_min_y {
        new_pos.y = vp_max_y - margin;
        should_warp = true;
    } else if current_cursor_pos.y >= vp_max_y {
        new_pos.y = vp_min_y + margin;
        should_warp = true;
    }

    let is_scale = matches!(modal.mode, Some(ModalTransformMode::Scale));
    if should_warp && !is_scale {
        window.set_cursor_position(Some(new_pos));
        modal.last_cursor_pos = new_pos;
        modal.just_warped = true;
    }

    let Some(mode) = modal.mode else { return };

    let cos_yaw = orbit.yaw.cos();
    let sin_yaw = orbit.yaw.sin();
    let cos_pitch = orbit.pitch.cos();
    let sin_pitch = orbit.pitch.sin();

    let cam_forward = -Vec3::new(cos_pitch * sin_yaw, sin_pitch, cos_pitch * cos_yaw).normalize();
    let cam_right = cam_forward.cross(Vec3::Y).normalize();
    let cam_up = cam_right.cross(cam_forward).normalize();

    let delta = modal.accumulated_delta;

    for state in modal.start_transforms.clone() {
        let Ok(mut transform) = transforms.get_mut(state.entity) else { continue };

        match mode {
            ModalTransformMode::Grab => {
                apply_grab(&mut transform, &state, &modal, delta, cam_right, cam_up);
            }
            ModalTransformMode::Rotate => {
                apply_rotate(&mut transform, &state, &modal, delta);
            }
            ModalTransformMode::Scale => {
                apply_scale(&mut transform, &state, &modal, current_cursor_pos);
            }
        }
    }
}

fn apply_grab(
    transform: &mut Transform,
    state: &EntityStartState,
    modal: &ModalTransformState,
    delta: Vec2,
    cam_right: Vec3,
    cam_up: Vec3,
) {
    if let Some(value) = modal.numeric_input.value() {
        let direction = match modal.axis_constraint {
            AxisConstraint::X => Vec3::X,
            AxisConstraint::Y => Vec3::Y,
            AxisConstraint::Z => Vec3::Z,
            AxisConstraint::PlaneYZ | AxisConstraint::PlaneXZ | AxisConstraint::PlaneXY => {
                (cam_right * delta.x.signum()).normalize_or_zero()
            }
            AxisConstraint::None => {
                let move_dir = (cam_right * delta.x - cam_up * delta.y).normalize_or_zero();
                if move_dir.length() > 0.0 { move_dir } else { cam_right }
            }
        };
        transform.translation = state.transform.translation + direction * value;
        return;
    }

    let sensitivity = 0.02;

    let world_delta = match modal.axis_constraint {
        AxisConstraint::None => {
            (cam_right * delta.x - cam_up * delta.y) * sensitivity
        }
        AxisConstraint::X => {
            let x_on_screen_right = Vec3::X.dot(cam_right);
            let x_on_screen_up = Vec3::X.dot(cam_up);
            let proj = delta.x * x_on_screen_right - delta.y * x_on_screen_up;
            Vec3::X * proj * sensitivity
        }
        AxisConstraint::Y => {
            let y_on_screen_right = Vec3::Y.dot(cam_right);
            let y_on_screen_up = Vec3::Y.dot(cam_up);
            let proj = delta.x * y_on_screen_right - delta.y * y_on_screen_up;
            Vec3::Y * proj * sensitivity
        }
        AxisConstraint::Z => {
            let z_on_screen_right = Vec3::Z.dot(cam_right);
            let z_on_screen_up = Vec3::Z.dot(cam_up);
            let proj = delta.x * z_on_screen_right - delta.y * z_on_screen_up;
            Vec3::Z * proj * sensitivity
        }
        AxisConstraint::PlaneYZ => {
            let move_yz = (cam_right * delta.x - cam_up * delta.y) * sensitivity;
            Vec3::new(0.0, move_yz.y, move_yz.z)
        }
        AxisConstraint::PlaneXZ => {
            let move_xz = (cam_right * delta.x - cam_up * delta.y) * sensitivity;
            Vec3::new(move_xz.x, 0.0, move_xz.z)
        }
        AxisConstraint::PlaneXY => {
            let move_xy = (cam_right * delta.x - cam_up * delta.y) * sensitivity;
            Vec3::new(move_xy.x, move_xy.y, 0.0)
        }
    };

    transform.translation = state.transform.translation + world_delta;
}

fn apply_rotate(
    transform: &mut Transform,
    state: &EntityStartState,
    modal: &ModalTransformState,
    delta: Vec2,
) {
    if let Some(degrees) = modal.numeric_input.value() {
        let radians = degrees.to_radians();
        let rotation = match modal.axis_constraint {
            AxisConstraint::X | AxisConstraint::PlaneYZ => Quat::from_rotation_x(radians),
            AxisConstraint::Y | AxisConstraint::PlaneXZ => Quat::from_rotation_y(radians),
            AxisConstraint::Z | AxisConstraint::PlaneXY | AxisConstraint::None => Quat::from_rotation_z(radians),
        };
        transform.rotation = rotation * state.transform.rotation;
        return;
    }

    let angle = (-delta.x + delta.y) * modal.sensitivity * 0.5;

    let rotation = match modal.axis_constraint {
        AxisConstraint::X | AxisConstraint::PlaneYZ => Quat::from_rotation_x(angle),
        AxisConstraint::Y | AxisConstraint::PlaneXZ => Quat::from_rotation_y(angle),
        AxisConstraint::Z | AxisConstraint::PlaneXY | AxisConstraint::None => Quat::from_rotation_z(angle),
    };

    transform.rotation = rotation * state.transform.rotation;
}

fn apply_scale(
    transform: &mut Transform,
    state: &EntityStartState,
    modal: &ModalTransformState,
    current_cursor: Vec2,
) {
    if let Some(factor) = modal.numeric_input.value() {
        let scale = axis_scale_vec(modal.axis_constraint, factor);
        transform.scale = state.transform.scale * scale;
        return;
    }

    let factor = if let Some(pivot) = modal.pivot_screen_pos {
        let v0 = modal.initial_cursor_pos - pivot;
        let v = current_cursor - pivot;
        let initial_dist = v0.length();
        if initial_dist < 1.0 {
            1.0
        } else {
            v.length() / initial_dist
        }
    } else {
        let dx = current_cursor.x - modal.initial_cursor_pos.x;
        1.0 + dx * modal.sensitivity * 0.1
    };

    let scale = axis_scale_vec(modal.axis_constraint, factor);
    transform.scale = state.transform.scale * scale;
}

fn axis_scale_vec(constraint: AxisConstraint, factor: f32) -> Vec3 {
    match constraint {
        AxisConstraint::None    => Vec3::splat(factor),
        AxisConstraint::X       => Vec3::new(factor, 1.0, 1.0),
        AxisConstraint::Y       => Vec3::new(1.0, factor, 1.0),
        AxisConstraint::Z       => Vec3::new(1.0, 1.0, factor),
        AxisConstraint::PlaneYZ => Vec3::new(1.0, factor, factor),
        AxisConstraint::PlaneXZ => Vec3::new(factor, 1.0, factor),
        AxisConstraint::PlaneXY => Vec3::new(factor, factor, 1.0),
    }
}

/// System to draw axis constraint overlay with gizmos
pub fn modal_transform_overlay_system(
    modal: Res<ModalTransformState>,
    transforms: Query<&Transform>,
    mut gizmos: Gizmos,
) {
    if !modal.active {
        return;
    }

    for state in &modal.start_transforms {
        let Ok(transform) = transforms.get(state.entity) else { continue };

        let pos = transform.translation;
        let line_length = 5.0;

        match modal.axis_constraint {
            AxisConstraint::None => {}
            AxisConstraint::X => {
                gizmos.line(pos - Vec3::X * line_length, pos + Vec3::X * line_length, AxisConstraint::X.color());
            }
            AxisConstraint::Y => {
                gizmos.line(pos - Vec3::Y * line_length, pos + Vec3::Y * line_length, AxisConstraint::Y.color());
            }
            AxisConstraint::Z => {
                gizmos.line(pos - Vec3::Z * line_length, pos + Vec3::Z * line_length, AxisConstraint::Z.color());
            }
            AxisConstraint::PlaneYZ => {
                gizmos.line(pos - Vec3::Y * line_length, pos + Vec3::Y * line_length, AxisConstraint::Y.color());
                gizmos.line(pos - Vec3::Z * line_length, pos + Vec3::Z * line_length, AxisConstraint::Z.color());
            }
            AxisConstraint::PlaneXZ => {
                gizmos.line(pos - Vec3::X * line_length, pos + Vec3::X * line_length, AxisConstraint::X.color());
                gizmos.line(pos - Vec3::Z * line_length, pos + Vec3::Z * line_length, AxisConstraint::Z.color());
            }
            AxisConstraint::PlaneXY => {
                gizmos.line(pos - Vec3::X * line_length, pos + Vec3::X * line_length, AxisConstraint::X.color());
                gizmos.line(pos - Vec3::Y * line_length, pos + Vec3::Y * line_length, AxisConstraint::Y.color());
            }
        }
    }
}
