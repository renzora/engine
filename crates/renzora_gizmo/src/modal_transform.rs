//! Blender-style modal transform system.
//!
//! Press G (grab), R (rotate), or S (scale) with an entity selected to enter modal transform mode.
//! Mouse movement applies the transform in real-time.
//! Press X/Y/Z to constrain to an axis, Shift+X/Y/Z for plane constraint.
//! Type numbers for precise values.
//! Enter/Left-click confirms, Escape/Right-click cancels.

use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};

use renzora::core::keybindings::{EditorAction, KeyBindings};
use renzora::core::viewport_types::ViewportState;
use renzora::core::InputFocusState;
use renzora_editor_framework::{EditorCamera, EditorSelection, HideInHierarchy};

use crate::OverlayGizmoGroup;

// ── Enums ───────────────────────────────────────────────────────────────────

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
    PlaneYZ, // Shift+X — exclude X axis
    PlaneXZ, // Shift+Y — exclude Y axis
    PlaneXY, // Shift+Z — exclude Z axis
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

// ── Numeric input buffer ────────────────────────────────────────────────────

/// Numeric input buffer for precise value entry during modal transforms.
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
        self.buffer
            .parse::<f32>()
            .ok()
            .map(|v| if self.negative { -v } else { v })
    }

    pub fn display(&self) -> String {
        let sign = if self.negative { "-" } else { "" };
        if self.buffer.is_empty() {
            return sign.to_string();
        }
        format!("{}{}", sign, self.buffer)
    }
}

// ── State ───────────────────────────────────────────────────────────────────

/// Stored transform for an entity at modal transform start.
#[derive(Clone)]
pub struct EntityStartState {
    pub entity: Entity,
    pub transform: Transform,
    /// The entity's parent world affine, captured at start so world-space
    /// deltas/rotations are written correctly into the local `Transform`.
    pub parent: bevy::math::Affine3A,
}

/// State for the modal transform system.
#[derive(Resource, Default)]
pub struct ModalTransformState {
    /// Whether modal transform is currently active.
    pub active: bool,
    /// Current transform mode.
    pub mode: Option<ModalTransformMode>,
    /// Axis constraint.
    pub axis_constraint: AxisConstraint,
    /// Numeric input buffer for precise values.
    pub numeric_input: NumericInputBuffer,
    /// Accumulated mouse delta.
    pub accumulated_delta: Vec2,
    /// Last cursor position.
    pub last_cursor_pos: Vec2,
    /// Starting transforms for all selected entities.
    pub start_transforms: Vec<EntityStartState>,
    /// Sensitivity multiplier.
    pub sensitivity: f32,
    /// Whether we just warped the cursor (skip one frame of delta).
    pub just_warped: bool,
    /// Pending grab mode for duplicate-and-move.
    pub pending_grab: bool,
    /// Screen-space pivot for scale visualization.
    pub pivot_screen_pos: Option<Vec2>,
    /// World-space pivot (selection AABB center) the rotate/scale pivots about,
    /// so the object transforms in place.
    pub pivot_world: Vec3,
    /// Cursor position when modal started.
    pub initial_cursor_pos: Vec2,
}

impl ModalTransformState {
    pub fn start(
        &mut self,
        mode: ModalTransformMode,
        cursor_pos: Vec2,
        entities: Vec<EntityStartState>,
    ) {
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

// ── Systems ─────────────────────────────────────────────────────────────────

/// Detect G/R/S key press and start modal transform mode.
pub fn modal_transform_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    selection: Res<EditorSelection>,
    viewport: Option<Res<ViewportState>>,
    viewport_settings: Option<Res<renzora::core::viewport_types::ViewportSettings>>,
    mut modal: ResMut<ModalTransformState>,
    transforms: Query<&Transform>,
    global_transforms: Query<&GlobalTransform>,
    // Same AABB-center pivot the tool gizmo uses, so the modal anchors on the
    // visible mesh — not the entity origin, which for baked-vertex GLBs (their
    // transform sits at world 0,0,0) is nowhere near what's on screen.
    aabbs: Query<(Option<&bevy::camera::primitives::Aabb>, &GlobalTransform), With<Mesh3d>>,
    children: Query<&Children>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut cursor_options: Query<&mut CursorOptions>,
    input_focus: Res<InputFocusState>,
    camera_query: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    hidden: Query<(), With<HideInHierarchy>>,
) {
    if modal.active {
        return;
    }
    // Modal G/R/S only operates on entity transforms while the viewport is
    // in Scene mode. Any other mode (Edit / Sculpt / Paint / Animate — current
    // or future) is owned by whichever plugin drives it.
    if !matches!(
        viewport_settings.as_deref().map(|s| s.viewport_mode),
        None | Some(renzora::core::viewport_types::ViewportMode::Scene)
    ) {
        return;
    }
    if keybindings.rebinding.is_some() {
        return;
    }
    if input_focus.egui_wants_keyboard {
        return;
    }
    if mouse_button.pressed(MouseButton::Right) {
        return;
    }

    let selected = selection.get_all();
    if selected.is_empty() {
        return;
    }

    // Check for pending grab (from duplicate and move)
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

    let Some(mode) = mode else { return };
    let Ok(mut window) = windows.single_mut() else {
        return;
    };

    // Compute pivot (average AABB center of selected entities)
    let mut avg_pos = Vec3::ZERO;
    let mut count = 0u32;
    for &entity in &selected {
        if hidden.get(entity).is_ok() {
            continue;
        }
        if let Ok(gt) = global_transforms.get(entity) {
            avg_pos += crate::compute_gizmo_pivot(entity, &aabbs, &children, gt);
            count += 1;
        }
    }
    if count == 0 {
        return;
    }
    avg_pos /= count as f32;

    let viewport_hovered = viewport.as_ref().is_none_or(|v| v.hovered);

    let pivot_screen_pos = camera_query
        .single()
        .ok()
        .and_then(|(camera, cam_transform)| {
            let vp = viewport.as_ref()?;
            let ndc = camera.world_to_ndc(cam_transform, avg_pos)?;
            if ndc.z < 0.0 || ndc.z > 1.0 {
                return None;
            }
            Some(Vec2::new(
                vp.screen_position.x + (ndc.x + 1.0) * 0.5 * vp.screen_size.x,
                vp.screen_position.y + (1.0 - ndc.y) * 0.5 * vp.screen_size.y,
            ))
        });

    let cursor_pos = if viewport_hovered {
        window.cursor_position().unwrap_or(Vec2::ZERO)
    } else {
        let Some(pivot) = pivot_screen_pos else {
            return;
        };
        window.set_cursor_position(Some(pivot));
        modal.just_warped = true;
        pivot
    };

    // Hide cursor for all modal modes (Grab / Rotate / Scale). The warped
    // pivot position is the anchor; the cursor itself isn't a useful
    // visual during modal drag.
    if let Ok(mut cursor) = cursor_options.single_mut() {
        cursor.visible = false;
    }
    let _ = mode;

    // Collect starting transforms (skip hidden entities), capturing each
    // entity's parent frame so writes go back into local space correctly.
    let start_transforms: Vec<EntityStartState> = selected
        .iter()
        .filter(|&&entity| hidden.get(entity).is_err())
        .filter_map(|&entity| {
            let t = *transforms.get(entity).ok()?;
            let parent = global_transforms
                .get(entity)
                .map(|gt| crate::transform_space::parent_affine(gt, &t))
                .unwrap_or(bevy::math::Affine3A::IDENTITY);
            Some(EntityStartState {
                entity,
                transform: t,
                parent,
            })
        })
        .collect();

    if !start_transforms.is_empty() {
        modal.start(mode, cursor_pos, start_transforms);
        modal.pivot_screen_pos = pivot_screen_pos;
        modal.pivot_world = avg_pos;
    }
}

/// Handle keyboard input during modal transform (axis, numbers, confirm/cancel).
pub fn modal_transform_keyboard_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut modal: ResMut<ModalTransformState>,
    mut transforms: Query<&mut Transform>,
    mut cursor_options: Query<&mut CursorOptions>,
    mut commands: Commands,
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
        (KeyCode::Digit0, '0'),
        (KeyCode::Digit1, '1'),
        (KeyCode::Digit2, '2'),
        (KeyCode::Digit3, '3'),
        (KeyCode::Digit4, '4'),
        (KeyCode::Digit5, '5'),
        (KeyCode::Digit6, '6'),
        (KeyCode::Digit7, '7'),
        (KeyCode::Digit8, '8'),
        (KeyCode::Digit9, '9'),
        (KeyCode::Numpad0, '0'),
        (KeyCode::Numpad1, '1'),
        (KeyCode::Numpad2, '2'),
        (KeyCode::Numpad3, '3'),
        (KeyCode::Numpad4, '4'),
        (KeyCode::Numpad5, '5'),
        (KeyCode::Numpad6, '6'),
        (KeyCode::Numpad7, '7'),
        (KeyCode::Numpad8, '8'),
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
    if keyboard.just_pressed(KeyCode::Enter)
        || keyboard.just_pressed(KeyCode::NumpadEnter)
        || mouse_button.just_pressed(MouseButton::Left)
    {
        // Record a TransformCmd per changed entity for undo.
        let mut records: Vec<(Entity, Transform, Transform)> = Vec::new();
        for state in &modal.start_transforms {
            let Ok(current) = transforms.get(state.entity) else {
                continue;
            };
            let old = state.transform;
            let new = *current;
            if old.translation == new.translation
                && old.rotation == new.rotation
                && old.scale == new.scale
            {
                continue;
            }
            records.push((state.entity, old, new));
        }
        if !records.is_empty() {
            commands.queue(move |world: &mut World| {
                for (entity, old, new) in records {
                    renzora_undo::record(
                        world,
                        renzora_undo::UndoContext::Scene,
                        Box::new(renzora_undo::TransformCmd { entity, old, new }),
                    );
                }
            });
        }
        modal.confirm();
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
        if let Ok(mut cursor) = cursor_options.single_mut() {
            cursor.visible = true;
        }
    }
}

/// Apply transform based on mouse delta or numeric input.
pub fn modal_transform_apply_system(
    mut modal: ResMut<ModalTransformState>,
    viewport: Option<Res<ViewportState>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut transforms: Query<&mut Transform>,
    camera_query: Query<&GlobalTransform, With<EditorCamera>>,
) {
    if !modal.active {
        return;
    }

    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let Some(current_cursor_pos) = window.cursor_position() else {
        return;
    };

    // Skip delta on warp frame
    if modal.just_warped {
        modal.last_cursor_pos = current_cursor_pos;
        modal.just_warped = false;
    } else {
        let frame_delta = current_cursor_pos - modal.last_cursor_pos;
        modal.accumulated_delta += frame_delta;
        modal.last_cursor_pos = current_cursor_pos;
    }

    // Cursor wrapping at viewport edges (skip for scale mode)
    let is_scale = matches!(modal.mode, Some(ModalTransformMode::Scale));
    if !is_scale {
        if let Some(vp) = viewport.as_ref() {
            let margin = 5.0;
            let vp_min_x = vp.screen_position.x + margin;
            let vp_max_x = vp.screen_position.x + vp.screen_size.x - margin;
            let vp_min_y = vp.screen_position.y + margin;
            let vp_max_y = vp.screen_position.y + vp.screen_size.y - margin;

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

            if should_warp {
                window.set_cursor_position(Some(new_pos));
                modal.last_cursor_pos = new_pos;
                modal.just_warped = true;
            }
        }
    }

    let Some(mode) = modal.mode else { return };

    // Camera directions from GlobalTransform
    let Ok(cam_gt) = camera_query.single() else {
        return;
    };
    let cam_right = cam_gt.right().as_vec3();
    let cam_up = cam_gt.up().as_vec3();

    let delta = modal.accumulated_delta;

    // Apply transform to all selected entities
    for state in modal.start_transforms.clone() {
        let Ok(mut transform) = transforms.get_mut(state.entity) else {
            continue;
        };

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

/// Draw axis constraint overlay lines during modal transform.
pub fn modal_transform_overlay_system(
    modal: Res<ModalTransformState>,
    transforms: Query<&Transform>,
    mut gizmos: Gizmos<OverlayGizmoGroup>,
) {
    if !modal.active {
        return;
    }

    for state in &modal.start_transforms {
        let Ok(transform) = transforms.get(state.entity) else {
            continue;
        };

        let pos = transform.translation;
        let line_length = 5.0;

        match modal.axis_constraint {
            AxisConstraint::None => {}
            AxisConstraint::X => {
                gizmos.line(
                    pos - Vec3::X * line_length,
                    pos + Vec3::X * line_length,
                    AxisConstraint::X.color(),
                );
            }
            AxisConstraint::Y => {
                gizmos.line(
                    pos - Vec3::Y * line_length,
                    pos + Vec3::Y * line_length,
                    AxisConstraint::Y.color(),
                );
            }
            AxisConstraint::Z => {
                gizmos.line(
                    pos - Vec3::Z * line_length,
                    pos + Vec3::Z * line_length,
                    AxisConstraint::Z.color(),
                );
            }
            AxisConstraint::PlaneYZ => {
                gizmos.line(
                    pos - Vec3::Y * line_length,
                    pos + Vec3::Y * line_length,
                    AxisConstraint::Y.color(),
                );
                gizmos.line(
                    pos - Vec3::Z * line_length,
                    pos + Vec3::Z * line_length,
                    AxisConstraint::Z.color(),
                );
            }
            AxisConstraint::PlaneXZ => {
                gizmos.line(
                    pos - Vec3::X * line_length,
                    pos + Vec3::X * line_length,
                    AxisConstraint::X.color(),
                );
                gizmos.line(
                    pos - Vec3::Z * line_length,
                    pos + Vec3::Z * line_length,
                    AxisConstraint::Z.color(),
                );
            }
            AxisConstraint::PlaneXY => {
                gizmos.line(
                    pos - Vec3::X * line_length,
                    pos + Vec3::X * line_length,
                    AxisConstraint::X.color(),
                );
                gizmos.line(
                    pos - Vec3::Y * line_length,
                    pos + Vec3::Y * line_length,
                    AxisConstraint::Y.color(),
                );
            }
        }
    }
}

// ── Transform application helpers ───────────────────────────────────────────

fn apply_grab(
    transform: &mut Transform,
    state: &EntityStartState,
    modal: &ModalTransformState,
    delta: Vec2,
    cam_right: Vec3,
    cam_up: Vec3,
) {
    // Numeric input: use value as distance along axis
    if let Some(value) = modal.numeric_input.value() {
        let direction = match modal.axis_constraint {
            AxisConstraint::X => Vec3::X,
            AxisConstraint::Y => Vec3::Y,
            AxisConstraint::Z => Vec3::Z,
            AxisConstraint::None => {
                let move_dir = (cam_right * delta.x - cam_up * delta.y).normalize_or_zero();
                if move_dir.length() > 0.0 {
                    move_dir
                } else {
                    cam_right
                }
            }
            _ => (cam_right * delta.x.signum()).normalize_or_zero(),
        };
        transform.translation = crate::transform_space::world_translation(
            state.transform.translation,
            direction * value,
            &state.parent,
        );
        return;
    }

    let sensitivity = 0.02;

    let world_delta = match modal.axis_constraint {
        AxisConstraint::None => (cam_right * delta.x - cam_up * delta.y) * sensitivity,
        AxisConstraint::X => {
            let x_on_right = Vec3::X.dot(cam_right);
            let x_on_up = Vec3::X.dot(cam_up);
            let proj = delta.x * x_on_right - delta.y * x_on_up;
            Vec3::X * proj * sensitivity
        }
        AxisConstraint::Y => {
            let y_on_right = Vec3::Y.dot(cam_right);
            let y_on_up = Vec3::Y.dot(cam_up);
            let proj = delta.x * y_on_right - delta.y * y_on_up;
            Vec3::Y * proj * sensitivity
        }
        AxisConstraint::Z => {
            let z_on_right = Vec3::Z.dot(cam_right);
            let z_on_up = Vec3::Z.dot(cam_up);
            let proj = delta.x * z_on_right - delta.y * z_on_up;
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

    transform.translation = crate::transform_space::world_translation(
        state.transform.translation,
        world_delta,
        &state.parent,
    );
}

fn apply_rotate(
    transform: &mut Transform,
    state: &EntityStartState,
    modal: &ModalTransformState,
    delta: Vec2,
) {
    // Numeric input: degrees
    if let Some(degrees) = modal.numeric_input.value() {
        let radians = degrees.to_radians();
        let rotation = match modal.axis_constraint {
            AxisConstraint::X | AxisConstraint::PlaneYZ => Quat::from_rotation_x(radians),
            AxisConstraint::Y | AxisConstraint::PlaneXZ => Quat::from_rotation_y(radians),
            AxisConstraint::Z | AxisConstraint::PlaneXY | AxisConstraint::None => {
                Quat::from_rotation_z(radians)
            }
        };
        crate::transform_space::pivot_rotation(
            transform,
            state.transform.translation,
            state.transform.rotation,
            state.transform.scale,
            rotation,
            modal.pivot_world,
            &state.parent,
        );
        return;
    }

    let angle = (-delta.x + delta.y) * modal.sensitivity * 0.5;

    let rotation = match modal.axis_constraint {
        AxisConstraint::X | AxisConstraint::PlaneYZ => Quat::from_rotation_x(angle),
        AxisConstraint::Y | AxisConstraint::PlaneXZ => Quat::from_rotation_y(angle),
        AxisConstraint::Z | AxisConstraint::PlaneXY | AxisConstraint::None => {
            Quat::from_rotation_z(angle)
        }
    };

    crate::transform_space::pivot_rotation(
        transform,
        state.transform.translation,
        state.transform.rotation,
        state.transform.scale,
        rotation,
        modal.pivot_world,
        &state.parent,
    );
}

fn apply_scale(
    transform: &mut Transform,
    state: &EntityStartState,
    modal: &ModalTransformState,
    current_cursor: Vec2,
) {
    // Numeric input: explicit factor
    if let Some(factor) = modal.numeric_input.value() {
        let new_scale = state.transform.scale * axis_scale_vec(modal.axis_constraint, factor);
        crate::transform_space::pivot_scale(
            transform,
            state.transform.translation,
            state.transform.rotation,
            state.transform.scale,
            new_scale,
            modal.pivot_world,
            &state.parent,
        );
        return;
    }

    // Distance-based scaling
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

    let new_scale = state.transform.scale * axis_scale_vec(modal.axis_constraint, factor);
    crate::transform_space::pivot_scale(
        transform,
        state.transform.translation,
        state.transform.rotation,
        state.transform.scale,
        new_scale,
        modal.pivot_world,
        &state.parent,
    );
}

fn axis_scale_vec(constraint: AxisConstraint, factor: f32) -> Vec3 {
    match constraint {
        AxisConstraint::None => Vec3::splat(factor),
        AxisConstraint::X => Vec3::new(factor, 1.0, 1.0),
        AxisConstraint::Y => Vec3::new(1.0, factor, 1.0),
        AxisConstraint::Z => Vec3::new(1.0, 1.0, factor),
        AxisConstraint::PlaneYZ => Vec3::new(1.0, factor, factor),
        AxisConstraint::PlaneXZ => Vec3::new(factor, 1.0, factor),
        AxisConstraint::PlaneXY => Vec3::new(factor, factor, 1.0),
    }
}

/// Sync modal transform state into the shared HUD resource so the viewport can render overlays.
pub fn sync_modal_hud(
    modal: Res<ModalTransformState>,
    mut hud: ResMut<renzora::core::ModalTransformHud>,
) {
    if !modal.active {
        hud.active = false;
        return;
    }
    hud.active = true;
    hud.mode = modal.mode.map_or("", |m| m.display_name());
    hud.is_scale = matches!(modal.mode, Some(ModalTransformMode::Scale));
    hud.pivot = modal.pivot_screen_pos.map(|v| [v.x, v.y]);
    hud.cursor = [modal.last_cursor_pos.x, modal.last_cursor_pos.y];
    hud.axis_name = modal.axis_constraint.display_name();
    let c = modal.axis_constraint.color();
    let rgba = c.to_srgba();
    hud.axis_color = [
        (rgba.red * 255.0) as u8,
        (rgba.green * 255.0) as u8,
        (rgba.blue * 255.0) as u8,
        (rgba.alpha * 255.0) as u8,
    ];
    hud.numeric_display = modal.numeric_input.display();

    // Reference circle + live factor for the scale HUD. The pivot the gesture
    // anchored on is the projected entity center (or the cursor at start if the
    // projection was unavailable). `ref_radius` is the start distance; the
    // factor mirrors `apply_scale`'s distance ratio (clamped when the start
    // distance is degenerate, e.g. the cursor was warped onto the pivot).
    let pivot = modal.pivot_screen_pos.unwrap_or(modal.initial_cursor_pos);
    let start_dist = (modal.initial_cursor_pos - pivot).length();
    let cur_dist = (modal.last_cursor_pos - pivot).length();
    hud.ref_radius = start_dist;
    hud.scale_factor = if start_dist < 1.0 {
        1.0
    } else {
        cur_dist / start_dist
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    fn start_state(transform: Transform) -> EntityStartState {
        EntityStartState {
            entity: Entity::PLACEHOLDER,
            transform,
            parent: bevy::math::Affine3A::IDENTITY,
        }
    }

    fn quat_approx_eq(a: Quat, b: Quat) -> bool {
        // q and -q are the same rotation.
        a.dot(b).abs() > 1.0 - 1e-5
    }

    // ── NumericInputBuffer ──────────────────────────────────────────────────

    #[test]
    fn numeric_buffer_builds_value_from_digits() {
        let mut buf = NumericInputBuffer::default();
        assert!(buf.is_empty());
        assert_eq!(buf.value(), None);

        buf.push_digit('1');
        buf.push_digit('2');
        buf.push_digit('x'); // non-digit ignored
        buf.push_digit('5');
        assert_eq!(buf.buffer, "125");
        assert_eq!(buf.value(), Some(125.0));
    }

    #[test]
    fn numeric_buffer_decimal_only_once_and_pads_leading_zero() {
        let mut buf = NumericInputBuffer::default();
        buf.push_decimal();
        assert_eq!(buf.buffer, "0.");
        buf.push_digit('5');
        buf.push_decimal(); // second decimal ignored
        assert_eq!(buf.buffer, "0.5");
        assert_eq!(buf.value(), Some(0.5));
    }

    #[test]
    fn numeric_buffer_negative_toggles_and_applies_to_value() {
        let mut buf = NumericInputBuffer::default();
        buf.push_digit('4');
        buf.toggle_negative();
        assert_eq!(buf.value(), Some(-4.0));
        assert_eq!(buf.display(), "-4");
        buf.toggle_negative();
        assert_eq!(buf.value(), Some(4.0));
    }

    #[test]
    fn numeric_buffer_backspace_restores_decimal_and_clears_sign() {
        let mut buf = NumericInputBuffer::default();
        buf.push_digit('1');
        buf.push_decimal();
        buf.backspace(); // removes '.', decimal becomes available again
        assert!(!buf.has_decimal);
        buf.push_decimal();
        assert_eq!(buf.buffer, "1.");

        let mut buf = NumericInputBuffer::default();
        buf.toggle_negative();
        assert_eq!(buf.display(), "-");
        buf.backspace(); // empty buffer: backspace clears the sign instead
        assert!(!buf.negative);
        assert_eq!(buf.display(), "");
    }

    // ── Enums & state machine ───────────────────────────────────────────────

    #[test]
    fn mode_and_constraint_display_names() {
        assert_eq!(ModalTransformMode::Grab.display_name(), "Grab");
        assert_eq!(ModalTransformMode::Rotate.display_name(), "Rotate");
        assert_eq!(ModalTransformMode::Scale.display_name(), "Scale");

        assert_eq!(AxisConstraint::None.display_name(), "");
        assert_eq!(AxisConstraint::X.display_name(), "X");
        assert_eq!(AxisConstraint::PlaneYZ.display_name(), "YZ");
        assert_eq!(AxisConstraint::PlaneXY.display_name(), "XY");
    }

    #[test]
    fn axis_constraint_color_matches_excluded_axis_pairing() {
        // A plane constraint shares the color of the axis it excludes.
        assert_eq!(AxisConstraint::X.color(), AxisConstraint::PlaneYZ.color());
        assert_eq!(AxisConstraint::Y.color(), AxisConstraint::PlaneXZ.color());
        assert_eq!(AxisConstraint::Z.color(), AxisConstraint::PlaneXY.color());
        assert_eq!(AxisConstraint::None.color(), Color::WHITE);
    }

    #[test]
    fn modal_state_start_resets_per_gesture_fields() {
        let mut state = ModalTransformState {
            axis_constraint: AxisConstraint::Z,
            accumulated_delta: Vec2::splat(50.0),
            ..Default::default()
        };
        state.numeric_input.push_digit('7');

        let cursor = Vec2::new(3.0, 4.0);
        state.start(
            ModalTransformMode::Grab,
            cursor,
            vec![start_state(Transform::IDENTITY)],
        );

        assert!(state.active);
        assert_eq!(state.mode, Some(ModalTransformMode::Grab));
        assert_eq!(state.axis_constraint, AxisConstraint::None);
        assert!(state.numeric_input.is_empty());
        assert_eq!(state.accumulated_delta, Vec2::ZERO);
        assert_eq!(state.last_cursor_pos, cursor);
        assert_eq!(state.initial_cursor_pos, cursor);
        assert_eq!(state.start_transforms.len(), 1);
    }

    #[test]
    fn modal_state_set_axis_toggles_same_constraint_off() {
        let mut state = ModalTransformState::default();
        state.set_axis(AxisConstraint::X);
        assert_eq!(state.axis_constraint, AxisConstraint::X);
        // Pressing the same axis again clears the constraint.
        state.set_axis(AxisConstraint::X);
        assert_eq!(state.axis_constraint, AxisConstraint::None);
        // Switching to a different axis replaces it directly.
        state.set_axis(AxisConstraint::Y);
        state.set_axis(AxisConstraint::PlaneXY);
        assert_eq!(state.axis_constraint, AxisConstraint::PlaneXY);
    }

    // ── apply_grab ──────────────────────────────────────────────────────────

    #[test]
    fn apply_grab_numeric_moves_along_constrained_axis() {
        let mut modal = ModalTransformState::default();
        modal.numeric_input.push_digit('5');
        modal.axis_constraint = AxisConstraint::Y;

        let start = start_state(Transform::from_xyz(1.0, 2.0, 3.0));
        let mut transform = Transform::IDENTITY;
        apply_grab(&mut transform, &start, &modal, Vec2::ZERO, Vec3::X, Vec3::Y);
        assert_eq!(transform.translation, Vec3::new(1.0, 7.0, 3.0));

        // Negative value moves the other way.
        modal.numeric_input.toggle_negative();
        apply_grab(&mut transform, &start, &modal, Vec2::ZERO, Vec3::X, Vec3::Y);
        assert_eq!(transform.translation, Vec3::new(1.0, -3.0, 3.0));
    }

    #[test]
    fn apply_grab_mouse_delta_free_move_uses_camera_axes() {
        let modal = ModalTransformState::default(); // constraint None, no numeric
        let start = start_state(Transform::IDENTITY);
        let mut transform = Transform::IDENTITY;
        // Screen-right = world +X, screen-up = world +Y; screen Y is inverted.
        apply_grab(
            &mut transform,
            &start,
            &modal,
            Vec2::new(10.0, 20.0),
            Vec3::X,
            Vec3::Y,
        );
        let expected = Vec3::new(0.2, -0.4, 0.0); // (right*10 - up*20) * 0.02
        assert!((transform.translation - expected).length() < 1e-5);
    }

    #[test]
    fn apply_grab_plane_constraint_zeroes_excluded_axis() {
        let modal = ModalTransformState {
            axis_constraint: AxisConstraint::PlaneXZ,
            ..Default::default()
        };
        let start = start_state(Transform::IDENTITY);
        let mut transform = Transform::IDENTITY;
        apply_grab(
            &mut transform,
            &start,
            &modal,
            Vec2::new(10.0, 20.0),
            Vec3::X,
            Vec3::Y,
        );
        // The free-move delta would be (0.2, -0.4, 0.0); XZ plane drops Y.
        assert!((transform.translation - Vec3::new(0.2, 0.0, 0.0)).length() < 1e-5);
    }

    #[test]
    fn apply_grab_axis_constraint_projects_screen_delta() {
        let modal = ModalTransformState {
            axis_constraint: AxisConstraint::X,
            ..Default::default()
        };
        let start = start_state(Transform::IDENTITY);
        let mut transform = Transform::IDENTITY;
        apply_grab(
            &mut transform,
            &start,
            &modal,
            Vec2::new(10.0, 20.0),
            Vec3::X,
            Vec3::Y,
        );
        // X projects fully onto cam_right: proj = dx = 10 → 10 * 0.02 along X.
        assert!((transform.translation - Vec3::new(0.2, 0.0, 0.0)).length() < 1e-5);
    }

    // ── apply_rotate ────────────────────────────────────────────────────────

    #[test]
    fn apply_rotate_numeric_input_is_degrees_about_constrained_axis() {
        let mut modal = ModalTransformState::default();
        modal.numeric_input.push_digit('9');
        modal.numeric_input.push_digit('0');
        modal.axis_constraint = AxisConstraint::Y;

        let start = start_state(Transform::IDENTITY);
        let mut transform = Transform::IDENTITY;
        apply_rotate(&mut transform, &start, &modal, Vec2::ZERO);
        assert!(quat_approx_eq(
            transform.rotation,
            Quat::from_rotation_y(FRAC_PI_2)
        ));
    }

    #[test]
    fn apply_rotate_composes_onto_start_rotation() {
        let mut modal = ModalTransformState::default();
        modal.numeric_input.push_digit('9');
        modal.numeric_input.push_digit('0');
        modal.axis_constraint = AxisConstraint::X;

        let initial = Quat::from_rotation_x(FRAC_PI_2);
        let start = start_state(Transform::from_rotation(initial));
        let mut transform = Transform::IDENTITY;
        apply_rotate(&mut transform, &start, &modal, Vec2::ZERO);
        // 90° + 90° about X = 180° about X — applied to start, not accumulated.
        assert!(quat_approx_eq(
            transform.rotation,
            Quat::from_rotation_x(std::f32::consts::PI)
        ));
    }

    #[test]
    fn apply_rotate_mouse_delta_defaults_to_z_axis() {
        let modal = ModalTransformState {
            sensitivity: 0.01,
            ..Default::default()
        };
        let start = start_state(Transform::IDENTITY);
        let mut transform = Transform::IDENTITY;
        apply_rotate(&mut transform, &start, &modal, Vec2::new(10.0, 0.0));
        // angle = (-dx + dy) * sensitivity * 0.5 = -0.05 about Z.
        assert!(quat_approx_eq(
            transform.rotation,
            Quat::from_rotation_z(-0.05)
        ));
    }

    // ── apply_scale ─────────────────────────────────────────────────────────

    #[test]
    fn apply_scale_numeric_factor_multiplies_start_scale() {
        let mut modal = ModalTransformState::default();
        modal.numeric_input.push_digit('2');

        let start = start_state(Transform::from_scale(Vec3::new(1.0, 2.0, 3.0)));
        let mut transform = Transform::IDENTITY;
        apply_scale(&mut transform, &start, &modal, Vec2::ZERO);
        assert_eq!(transform.scale, Vec3::new(2.0, 4.0, 6.0));

        // Single-axis constraint scales only that component.
        modal.axis_constraint = AxisConstraint::X;
        apply_scale(&mut transform, &start, &modal, Vec2::ZERO);
        assert_eq!(transform.scale, Vec3::new(2.0, 2.0, 3.0));
    }

    #[test]
    fn apply_scale_distance_based_factor_from_pivot() {
        let modal = ModalTransformState {
            pivot_screen_pos: Some(Vec2::new(100.0, 100.0)),
            initial_cursor_pos: Vec2::new(110.0, 100.0), // 10px from pivot
            ..Default::default()
        };
        let start = start_state(Transform::from_scale(Vec3::ONE));
        let mut transform = Transform::IDENTITY;
        // Cursor now 20px from pivot → factor 2.
        apply_scale(&mut transform, &start, &modal, Vec2::new(120.0, 100.0));
        assert!((transform.scale - Vec3::splat(2.0)).length() < 1e-5);
    }

    #[test]
    fn apply_scale_degenerate_pivot_distance_clamps_to_one() {
        // Initial cursor essentially on the pivot — factor must stay 1.0
        // instead of exploding.
        let modal = ModalTransformState {
            pivot_screen_pos: Some(Vec2::new(100.0, 100.0)),
            initial_cursor_pos: Vec2::new(100.5, 100.0),
            ..Default::default()
        };
        let start = start_state(Transform::from_scale(Vec3::splat(3.0)));
        let mut transform = Transform::IDENTITY;
        apply_scale(&mut transform, &start, &modal, Vec2::new(500.0, 100.0));
        assert_eq!(transform.scale, Vec3::splat(3.0));
    }

    #[test]
    fn apply_scale_without_pivot_uses_horizontal_delta() {
        let modal = ModalTransformState {
            pivot_screen_pos: None,
            initial_cursor_pos: Vec2::new(100.0, 100.0),
            sensitivity: 0.01,
            ..Default::default()
        };
        let start = start_state(Transform::from_scale(Vec3::ONE));
        let mut transform = Transform::IDENTITY;
        apply_scale(&mut transform, &start, &modal, Vec2::new(150.0, 100.0));
        // factor = 1 + 50 * 0.01 * 0.1 = 1.05
        assert!((transform.scale - Vec3::splat(1.05)).length() < 1e-5);
    }

    #[test]
    fn axis_scale_vec_covers_all_constraints() {
        let f = 2.0;
        assert_eq!(axis_scale_vec(AxisConstraint::None, f), Vec3::splat(2.0));
        assert_eq!(axis_scale_vec(AxisConstraint::X, f), Vec3::new(2.0, 1.0, 1.0));
        assert_eq!(axis_scale_vec(AxisConstraint::Y, f), Vec3::new(1.0, 2.0, 1.0));
        assert_eq!(axis_scale_vec(AxisConstraint::Z, f), Vec3::new(1.0, 1.0, 2.0));
        assert_eq!(
            axis_scale_vec(AxisConstraint::PlaneYZ, f),
            Vec3::new(1.0, 2.0, 2.0)
        );
        assert_eq!(
            axis_scale_vec(AxisConstraint::PlaneXZ, f),
            Vec3::new(2.0, 1.0, 2.0)
        );
        assert_eq!(
            axis_scale_vec(AxisConstraint::PlaneXY, f),
            Vec3::new(2.0, 2.0, 1.0)
        );
    }
}
