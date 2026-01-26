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

use crate::commands::{CommandHistory, SetTransformCommand, queue_command};
use crate::core::{InputFocusState, SelectionState, ViewportState, OrbitCameraState, KeyBindings, EditorAction};

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
    PlaneYZ,  // Shift+X - exclude X axis
    PlaneXZ,  // Shift+Y - exclude Y axis
    PlaneXY,  // Shift+Z - exclude Z axis
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
            AxisConstraint::X | AxisConstraint::PlaneYZ => Color::srgb(0.93, 0.3, 0.36),  // Red
            AxisConstraint::Y | AxisConstraint::PlaneXZ => Color::srgb(0.55, 0.79, 0.25), // Green
            AxisConstraint::Z | AxisConstraint::PlaneXY => Color::srgb(0.27, 0.54, 1.0),  // Blue
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
            // If buffer is empty but negative is set, clear the negative
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
            // Show just the minus sign if negative is toggled but no digits yet
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
    /// Whether modal transform is currently active
    pub active: bool,
    /// Current transform mode
    pub mode: Option<ModalTransformMode>,
    /// Axis constraint
    pub axis_constraint: AxisConstraint,
    /// Numeric input buffer for precise values
    pub numeric_input: NumericInputBuffer,
    /// Accumulated mouse delta (for infinite movement with cursor wrapping)
    pub accumulated_delta: Vec2,
    /// Last cursor position (for calculating frame delta)
    pub last_cursor_pos: Vec2,
    /// Starting transforms for all selected entities (for undo/cancel)
    pub start_transforms: Vec<EntityStartState>,
    /// Sensitivity multiplier for mouse movement
    pub sensitivity: f32,
    /// Whether we just warped the cursor (skip one frame of delta)
    pub just_warped: bool,
}

impl ModalTransformState {
    /// Start a modal transform operation
    pub fn start(&mut self, mode: ModalTransformMode, cursor_pos: Vec2, entities: Vec<EntityStartState>) {
        self.active = true;
        self.mode = Some(mode);
        self.axis_constraint = AxisConstraint::None;
        self.numeric_input.clear();
        self.accumulated_delta = Vec2::ZERO;
        self.last_cursor_pos = cursor_pos;
        self.start_transforms = entities;
        self.sensitivity = 0.01; // Default sensitivity
        self.just_warped = false;
    }

    /// Cancel and restore original transforms
    pub fn cancel(&mut self) {
        self.active = false;
        self.mode = None;
        // Note: transforms are restored by the system
    }

    /// Confirm the transform
    pub fn confirm(&mut self) {
        self.active = false;
        self.mode = None;
        self.start_transforms.clear();
    }

    /// Set axis constraint
    pub fn set_axis(&mut self, axis: AxisConstraint) {
        // Toggle off if same axis pressed again
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
    selection: Res<SelectionState>,
    viewport: Res<ViewportState>,
    mut modal: ResMut<ModalTransformState>,
    transforms: Query<&Transform>,
    windows: Query<&Window>,
    mut cursor_options: Query<&mut CursorOptions>,
    input_focus: Res<InputFocusState>,
) {
    // Don't start new modal if one is active
    if modal.active {
        return;
    }

    // Don't process while rebinding keys
    if keybindings.rebinding.is_some() {
        return;
    }

    // Don't process when a text input is focused
    if input_focus.egui_wants_keyboard {
        return;
    }

    // Need at least one selected entity
    let selected = selection.get_all_selected();
    if selected.is_empty() {
        return;
    }

    // Only respond when viewport is hovered
    if !viewport.hovered {
        return;
    }

    // Check for modal transform keybindings
    let mode = if keybindings.just_pressed(EditorAction::ModalGrab, &keyboard) {
        Some(ModalTransformMode::Grab)
    } else if keybindings.just_pressed(EditorAction::ModalRotate, &keyboard) {
        Some(ModalTransformMode::Rotate)
    } else if keybindings.just_pressed(EditorAction::ModalScale, &keyboard) {
        Some(ModalTransformMode::Scale)
    } else {
        None
    };

    if let Some(mode) = mode {
        // Get cursor position
        let cursor_pos = windows
            .single()
            .ok()
            .and_then(|w| w.cursor_position())
            .unwrap_or(Vec2::ZERO);

        // Hide cursor
        if let Ok(mut cursor) = cursor_options.single_mut() {
            cursor.visible = false;
        }

        // Collect starting transforms for all selected entities
        let start_transforms: Vec<EntityStartState> = selected
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
        }
    }
}

/// System to handle keyboard input during modal transform (axis, numbers, confirm/cancel)
pub fn modal_transform_keyboard_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut modal: ResMut<ModalTransformState>,
    mut transforms: Query<&mut Transform>,
    mut command_history: ResMut<CommandHistory>,
    time: Res<Time>,
    mut cursor_options: Query<&mut CursorOptions>,
) {
    if !modal.active {
        return;
    }

    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

    // Axis constraints
    if keyboard.just_pressed(KeyCode::KeyX) {
        if shift {
            modal.set_axis(AxisConstraint::PlaneYZ);
        } else {
            modal.set_axis(AxisConstraint::X);
        }
    }
    if keyboard.just_pressed(KeyCode::KeyY) {
        if shift {
            modal.set_axis(AxisConstraint::PlaneXZ);
        } else {
            modal.set_axis(AxisConstraint::Y);
        }
    }
    if keyboard.just_pressed(KeyCode::KeyZ) {
        if shift {
            modal.set_axis(AxisConstraint::PlaneXY);
        } else {
            modal.set_axis(AxisConstraint::Z);
        }
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

    // Decimal point
    if keyboard.just_pressed(KeyCode::Period) || keyboard.just_pressed(KeyCode::NumpadDecimal) {
        modal.numeric_input.push_decimal();
    }

    // Negative toggle
    if keyboard.just_pressed(KeyCode::Minus) || keyboard.just_pressed(KeyCode::NumpadSubtract) {
        modal.numeric_input.toggle_negative();
    }

    // Backspace
    if keyboard.just_pressed(KeyCode::Backspace) {
        modal.numeric_input.backspace();
    }

    // Confirm with Enter or left-click
    if keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::NumpadEnter)
        || mouse_button.just_pressed(MouseButton::Left)
    {
        // Create undo commands for all transformed entities
        let timestamp = time.elapsed_secs_f64();
        for state in &modal.start_transforms {
            if let Ok(transform) = transforms.get(state.entity) {
                let cmd = SetTransformCommand {
                    entity: state.entity,
                    new_transform: *transform,
                    old_transform: Some(state.transform),
                    timestamp,
                };
                queue_command(&mut command_history, Box::new(cmd));
            }
        }
        modal.confirm();
        // Show cursor again
        if let Ok(mut cursor) = cursor_options.single_mut() {
            cursor.visible = true;
        }
        return;
    }

    // Cancel with Escape or right-click
    if keyboard.just_pressed(KeyCode::Escape) || mouse_button.just_pressed(MouseButton::Right) {
        // Restore original transforms
        for state in &modal.start_transforms {
            if let Ok(mut transform) = transforms.get_mut(state.entity) {
                *transform = state.transform;
            }
        }
        modal.cancel();
        // Show cursor again
        if let Ok(mut cursor) = cursor_options.single_mut() {
            cursor.visible = true;
        }
    }
}

/// System to apply transform based on mouse delta or numeric input
pub fn modal_transform_apply_system(
    mut modal: ResMut<ModalTransformState>,
    orbit: Res<OrbitCameraState>,
    viewport: Res<ViewportState>,
    mut windows: Query<&mut Window>,
    mut transforms: Query<&mut Transform>,
) {
    if !modal.active {
        return;
    }

    // Get current cursor position and handle wrapping
    let Ok(mut window) = windows.single_mut() else { return };
    let Some(current_cursor_pos) = window.cursor_position() else { return };

    // If we just warped, skip this frame's delta calculation
    if modal.just_warped {
        modal.last_cursor_pos = current_cursor_pos;
        modal.just_warped = false;
    } else {
        // Calculate frame delta and add to accumulated
        let frame_delta = current_cursor_pos - modal.last_cursor_pos;
        modal.accumulated_delta += frame_delta;
        modal.last_cursor_pos = current_cursor_pos;
    }

    // Check for cursor wrapping at viewport edges
    let margin = 5.0;
    let vp_min_x = viewport.position[0] + margin;
    let vp_max_x = viewport.position[0] + viewport.size[0] - margin;
    let vp_min_y = viewport.position[1] + margin;
    let vp_max_y = viewport.position[1] + viewport.size[1] - margin;

    let mut new_pos = current_cursor_pos;
    let mut should_warp = false;

    // Wrap horizontally
    if current_cursor_pos.x <= vp_min_x {
        new_pos.x = vp_max_x - margin;
        should_warp = true;
    } else if current_cursor_pos.x >= vp_max_x {
        new_pos.x = vp_min_x + margin;
        should_warp = true;
    }

    // Wrap vertically
    if current_cursor_pos.y <= vp_min_y {
        new_pos.y = vp_max_y - margin;
        should_warp = true;
    } else if current_cursor_pos.y >= vp_max_y {
        new_pos.y = vp_min_y + margin;
        should_warp = true;
    }

    if should_warp {
        window.set_cursor_position(Some(new_pos));
        modal.last_cursor_pos = new_pos;
        modal.just_warped = true;
    }

    let Some(mode) = modal.mode else { return };

    // Calculate camera directions matching the orbit camera setup
    // Camera looks from cam_pos toward orbit.focus
    // cam_pos = focus + offset, so forward = -offset direction
    let cos_yaw = orbit.yaw.cos();
    let sin_yaw = orbit.yaw.sin();
    let cos_pitch = orbit.pitch.cos();
    let sin_pitch = orbit.pitch.sin();

    // Camera forward (direction camera is looking, toward focus)
    let cam_forward = -Vec3::new(
        cos_pitch * sin_yaw,
        sin_pitch,
        cos_pitch * cos_yaw,
    ).normalize();

    // Camera right (horizontal, perpendicular to forward)
    let cam_right = cam_forward.cross(Vec3::Y).normalize();

    // Camera up (perpendicular to forward and right)
    let cam_up = cam_right.cross(cam_forward).normalize();

    // Get accumulated mouse delta
    let delta = modal.accumulated_delta;

    // Apply transform to all selected entities
    for state in modal.start_transforms.clone() {
        let Ok(mut transform) = transforms.get_mut(state.entity) else {
            continue;
        };

        match mode {
            ModalTransformMode::Grab => {
                apply_grab(&mut transform, &state, &modal, delta, cam_right, cam_up, &viewport);
            }
            ModalTransformMode::Rotate => {
                apply_rotate(&mut transform, &state, &modal, delta);
            }
            ModalTransformMode::Scale => {
                apply_scale(&mut transform, &state, &modal, delta);
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
    _viewport: &ViewportState,
) {
    // Check for numeric input
    if let Some(value) = modal.numeric_input.value() {
        // Use numeric value as distance along axis
        let direction = match modal.axis_constraint {
            AxisConstraint::X => Vec3::X,
            AxisConstraint::Y => Vec3::Y,
            AxisConstraint::Z => Vec3::Z,
            AxisConstraint::PlaneYZ => (cam_right * delta.x.signum()).normalize_or_zero(),
            AxisConstraint::PlaneXZ => (cam_right * delta.x.signum()).normalize_or_zero(),
            AxisConstraint::PlaneXY => (cam_right * delta.x.signum()).normalize_or_zero(),
            AxisConstraint::None => {
                // Move along camera plane based on mouse direction
                let move_dir = (cam_right * delta.x - cam_up * delta.y).normalize_or_zero();
                if move_dir.length() > 0.0 { move_dir } else { cam_right }
            }
        };
        transform.translation = state.transform.translation + direction * value;
        return;
    }

    // Mouse-based movement - use a fixed sensitivity for predictable movement
    // Screen pixels to world units ratio (adjust based on feel)
    let sensitivity = 0.02;

    // Calculate world-space movement based on screen delta
    // In screen coords: +X is right, +Y is DOWN
    // In world coords with cam_up: +cam_up is UP
    // So we need to negate delta.y when applying to cam_up
    let world_delta = match modal.axis_constraint {
        AxisConstraint::None => {
            // Free movement on camera plane
            // delta.x moves right (positive cam_right)
            // delta.y moves down on screen, so -delta.y moves up in world (positive cam_up)
            (cam_right * delta.x - cam_up * delta.y) * sensitivity
        }
        AxisConstraint::X => {
            // Project screen movement onto X axis
            // Use the component of screen motion that aligns with X axis direction
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
            // Movement on YZ plane (exclude X)
            let move_yz = (cam_right * delta.x - cam_up * delta.y) * sensitivity;
            Vec3::new(0.0, move_yz.y, move_yz.z)
        }
        AxisConstraint::PlaneXZ => {
            // Movement on XZ plane (exclude Y)
            let move_xz = (cam_right * delta.x - cam_up * delta.y) * sensitivity;
            Vec3::new(move_xz.x, 0.0, move_xz.z)
        }
        AxisConstraint::PlaneXY => {
            // Movement on XY plane (exclude Z)
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
    // Check for numeric input (degrees)
    if let Some(degrees) = modal.numeric_input.value() {
        let radians = degrees.to_radians();
        let rotation = match modal.axis_constraint {
            AxisConstraint::X | AxisConstraint::PlaneYZ => Quat::from_rotation_x(radians),
            AxisConstraint::Y | AxisConstraint::PlaneXZ => Quat::from_rotation_y(radians),
            AxisConstraint::Z | AxisConstraint::PlaneXY | AxisConstraint::None => {
                Quat::from_rotation_z(radians)
            }
        };
        transform.rotation = rotation * state.transform.rotation;
        return;
    }

    // Mouse-based rotation
    // Negative delta.x (move left) = positive rotation (counter-clockwise)
    // Positive delta.x (move right) = negative rotation (clockwise)
    let angle = (-delta.x + delta.y) * modal.sensitivity * 0.5;

    let rotation = match modal.axis_constraint {
        AxisConstraint::X | AxisConstraint::PlaneYZ => Quat::from_rotation_x(angle),
        AxisConstraint::Y | AxisConstraint::PlaneXZ => Quat::from_rotation_y(angle),
        AxisConstraint::Z | AxisConstraint::PlaneXY | AxisConstraint::None => {
            Quat::from_rotation_z(angle)
        }
    };

    transform.rotation = rotation * state.transform.rotation;
}

fn apply_scale(
    transform: &mut Transform,
    state: &EntityStartState,
    modal: &ModalTransformState,
    delta: Vec2,
) {
    // Check for numeric input (scale factor)
    if let Some(factor) = modal.numeric_input.value() {
        let factor = factor.max(0.001); // Prevent negative/zero scale
        let scale = match modal.axis_constraint {
            AxisConstraint::None => Vec3::splat(factor),
            AxisConstraint::X => Vec3::new(factor, 1.0, 1.0),
            AxisConstraint::Y => Vec3::new(1.0, factor, 1.0),
            AxisConstraint::Z => Vec3::new(1.0, 1.0, factor),
            AxisConstraint::PlaneYZ => Vec3::new(1.0, factor, factor),
            AxisConstraint::PlaneXZ => Vec3::new(factor, 1.0, factor),
            AxisConstraint::PlaneXY => Vec3::new(factor, factor, 1.0),
        };
        transform.scale = state.transform.scale * scale;
        return;
    }

    // Mouse-based scaling
    // Use horizontal movement for scale (right = bigger, left = smaller)
    let factor = 1.0 + delta.x * modal.sensitivity * 0.1;
    let factor = factor.max(0.001);

    let scale = match modal.axis_constraint {
        AxisConstraint::None => Vec3::splat(factor),
        AxisConstraint::X => Vec3::new(factor, 1.0, 1.0),
        AxisConstraint::Y => Vec3::new(1.0, factor, 1.0),
        AxisConstraint::Z => Vec3::new(1.0, 1.0, factor),
        AxisConstraint::PlaneYZ => Vec3::new(1.0, factor, factor),
        AxisConstraint::PlaneXZ => Vec3::new(factor, 1.0, factor),
        AxisConstraint::PlaneXY => Vec3::new(factor, factor, 1.0),
    };

    transform.scale = state.transform.scale * scale;
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

    // Draw axis lines from selected entity positions
    for state in &modal.start_transforms {
        let Ok(transform) = transforms.get(state.entity) else {
            continue;
        };

        let pos = transform.translation;
        let line_length = 5.0;

        // Draw constraint axis lines
        match modal.axis_constraint {
            AxisConstraint::None => {
                // No constraint visualization
            }
            AxisConstraint::X => {
                let color = AxisConstraint::X.color();
                gizmos.line(pos - Vec3::X * line_length, pos + Vec3::X * line_length, color);
            }
            AxisConstraint::Y => {
                let color = AxisConstraint::Y.color();
                gizmos.line(pos - Vec3::Y * line_length, pos + Vec3::Y * line_length, color);
            }
            AxisConstraint::Z => {
                let color = AxisConstraint::Z.color();
                gizmos.line(pos - Vec3::Z * line_length, pos + Vec3::Z * line_length, color);
            }
            AxisConstraint::PlaneYZ => {
                // Draw Y and Z axes
                gizmos.line(pos - Vec3::Y * line_length, pos + Vec3::Y * line_length, AxisConstraint::Y.color());
                gizmos.line(pos - Vec3::Z * line_length, pos + Vec3::Z * line_length, AxisConstraint::Z.color());
            }
            AxisConstraint::PlaneXZ => {
                // Draw X and Z axes
                gizmos.line(pos - Vec3::X * line_length, pos + Vec3::X * line_length, AxisConstraint::X.color());
                gizmos.line(pos - Vec3::Z * line_length, pos + Vec3::Z * line_length, AxisConstraint::Z.color());
            }
            AxisConstraint::PlaneXY => {
                // Draw X and Y axes
                gizmos.line(pos - Vec3::X * line_length, pos + Vec3::X * line_length, AxisConstraint::X.color());
                gizmos.line(pos - Vec3::Y * line_length, pos + Vec3::Y * line_length, AxisConstraint::Y.color());
            }
        }
    }
}
