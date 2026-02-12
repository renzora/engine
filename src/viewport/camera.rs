use bevy::prelude::*;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::core::{EditorEntity, InputFocusState, ViewportCamera, KeyBindings, EditorAction, SelectionState, ViewportState, OrbitCameraState, EditorSettings, ProjectionMode, MainCamera};
use crate::gizmo::{ModalTransformState, GizmoState};

pub fn camera_controller(
    selection: Res<SelectionState>,
    mut viewport: ResMut<ViewportState>,
    mut orbit: ResMut<OrbitCameraState>,
    settings: Res<EditorSettings>,
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut scroll_events: MessageReader<MouseWheel>,
    mut camera_query: Query<&mut Transform, With<ViewportCamera>>,
    entity_query: Query<&Transform, (With<EditorEntity>, Without<ViewportCamera>)>,
    modal: Res<ModalTransformState>,
    input_focus: Res<InputFocusState>,
    mut window_query: Query<(&mut CursorOptions, &Window), With<PrimaryWindow>>,
    gizmo: Res<GizmoState>,
) {
    let Ok(mut transform) = camera_query.single_mut() else {
        return;
    };

    // Disable camera movement during modal transform
    if modal.active {
        mouse_motion.clear();
        scroll_events.clear();
        return;
    }

    // Don't process keyboard shortcuts when a text input is focused
    // (mouse controls still work)
    let keyboard_enabled = !input_focus.egui_wants_keyboard;

    // Focus on selected entity (works even when not hovering viewport)
    if keyboard_enabled && keybindings.just_pressed(EditorAction::FocusSelected, &keyboard) {
        if let Some(selected) = selection.selected_entity {
            if let Ok(target_transform) = entity_query.get(selected) {
                orbit.focus = target_transform.translation;
                // Optionally adjust distance based on object bounds (simplified to fixed distance)
                orbit.distance = 5.0;
            }
        }
    }

    if !viewport.hovered {
        mouse_motion.clear();
        scroll_events.clear();
        return;
    }

    // Get camera settings (slow_mult applied later after Ctrl check)
    let cam_settings = &settings.camera_settings;
    let base_look_speed = cam_settings.look_sensitivity * 0.01;
    let base_orbit_speed = cam_settings.orbit_sensitivity * 0.01;
    let base_pan_speed = cam_settings.pan_sensitivity * 0.01;
    let base_zoom_speed = cam_settings.zoom_sensitivity;
    let base_move_speed = cam_settings.move_speed;
    let invert_y = if cam_settings.invert_y { -1.0 } else { 1.0 };
    let delta = time.delta_secs();

    let left_pressed = mouse_button.pressed(MouseButton::Left);
    let middle_pressed = mouse_button.pressed(MouseButton::Middle);
    let right_pressed = mouse_button.pressed(MouseButton::Right);
    let left_just_pressed = mouse_button.just_pressed(MouseButton::Left);
    let middle_just_pressed = mouse_button.just_pressed(MouseButton::Middle);
    let right_just_pressed = mouse_button.just_pressed(MouseButton::Right);
    let left_just_released = mouse_button.just_released(MouseButton::Left);
    let right_just_released = mouse_button.just_released(MouseButton::Right);
    let alt_held = keyboard.pressed(KeyCode::AltLeft) || keyboard.pressed(KeyCode::AltRight);
    let ctrl_held = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);

    // Slow modifier when Ctrl is held (0.25x speed)
    let slow_mult = if ctrl_held { 0.25 } else { 1.0 };

    // Distance-relative speed scaling (normalized around distance of 10 units)
    let distance_mult = if cam_settings.distance_relative_speed {
        (orbit.distance / 10.0).max(0.1)
    } else {
        1.0
    };

    // Apply slow modifier and distance scaling to speeds
    let look_speed = base_look_speed * slow_mult;
    let orbit_speed = base_orbit_speed * slow_mult;
    let pan_speed = base_pan_speed * slow_mult * distance_mult;
    let zoom_speed = base_zoom_speed * slow_mult * distance_mult;
    let move_speed = base_move_speed * slow_mult * distance_mult;

    // Handle cursor visibility and grab mode for camera dragging
    // Only start camera drag when click ORIGINATES inside the viewport
    // Don't start camera drag if gizmo is being dragged or hovered (for left click)
    let middle_just_released = mouse_button.just_released(MouseButton::Middle);
    let gizmo_hovered_or_dragging = gizmo.is_dragging || gizmo.hovered_axis.is_some();
    let left_click_drag_disabled = viewport.disable_left_click_drag || !cam_settings.left_click_pan;
    if let Ok((mut cursor, window)) = window_query.single_mut() {
        // Capture cursor position on right-click press for click-vs-drag detection
        if right_just_pressed && viewport.hovered && !alt_held {
            if let Some(pos) = window.cursor_position() {
                viewport.context_menu_pos = None; // close any existing menu
                viewport.right_click_origin = Some(bevy::math::Vec2::new(pos.x, pos.y));
                viewport.right_click_moved = false;
            }
        }

        // Start camera drag only when mouse button is first pressed while inside viewport
        // For left click, don't start if gizmo is hovered or being dragged
        // Middle/right click always work for camera control
        // Left-click drag can be disabled (e.g., in terrain layout for brush tools)
        let start_drag = viewport.hovered && !alt_held && (
            (left_just_pressed && !gizmo_hovered_or_dragging && !left_click_drag_disabled) ||
            middle_just_pressed ||
            right_just_pressed
        );

        if start_drag {
            cursor.visible = false;
            cursor.grab_mode = CursorGrabMode::Locked;
            viewport.camera_dragging = true;
        } else if left_just_released || middle_just_released || right_just_released {
            cursor.visible = true;
            cursor.grab_mode = CursorGrabMode::None;
            viewport.camera_dragging = false;

            // Right-click without drag -> open context menu
            if right_just_released {
                if let Some(origin) = viewport.right_click_origin.take() {
                    if !viewport.right_click_moved {
                        viewport.context_menu_pos = Some(origin);
                        viewport.context_submenu = None;
                    }
                }
                viewport.right_click_moved = false;
            }
        }
    }

    // Scroll wheel - zoom (works when hovering, doesn't require drag)
    // Skip zoom when brush tools are active (scroll adjusts brush size instead)
    let brush_tool_active = matches!(gizmo.tool, crate::gizmo::EditorTool::TerrainSculpt | crate::gizmo::EditorTool::SurfacePaint);
    let mut scroll_changed = false;
    if !brush_tool_active {
        for ev in scroll_events.read() {
            orbit.distance -= ev.y * zoom_speed;
            orbit.distance = orbit.distance.clamp(0.5, 100.0);
            scroll_changed = true;
        }
    } else {
        scroll_events.clear();
    }

    // If scroll changed but no drag, just update camera position and return
    if scroll_changed && !viewport.camera_dragging {
        let cam_pos = orbit.focus
            + Vec3::new(
                orbit.distance * orbit.pitch.cos() * orbit.yaw.sin(),
                orbit.distance * orbit.pitch.sin(),
                orbit.distance * orbit.pitch.cos() * orbit.yaw.cos(),
            );
        transform.translation = cam_pos;
        transform.look_at(orbit.focus, Vec3::Y);
        mouse_motion.clear();
        return;
    }

    // Clear accumulated events on first frame of button press to prevent jump
    if left_just_pressed || middle_just_pressed || right_just_pressed {
        mouse_motion.clear();
        return;
    }

    // Only process mouse drag camera controls if the drag started inside the viewport
    if !viewport.camera_dragging {
        mouse_motion.clear();
        return;
    }

    // === UNREAL ENGINE STYLE CAMERA CONTROLS ===

    // Right mouse - Look around + WASD fly mode
    // Camera position stays fixed during look, WASD moves the camera
    if right_pressed {
        // First handle WASD movement
        if keyboard_enabled {
            let mut move_delta = Vec3::ZERO;

            // Get camera forward direction (includes pitch for true 3D movement)
            let forward = Vec3::new(
                orbit.pitch.cos() * orbit.yaw.sin(),
                orbit.pitch.sin(),
                orbit.pitch.cos() * orbit.yaw.cos(),
            ).normalize();

            // Get right direction (stays horizontal for intuitive strafing)
            let right_dir = Vec3::new(
                orbit.yaw.cos(),
                0.0,
                -orbit.yaw.sin(),
            ).normalize();

            // Forward/backward (W/S)
            if keybindings.pressed(EditorAction::CameraMoveForward, &keyboard) {
                move_delta -= forward;
            }
            if keybindings.pressed(EditorAction::CameraMoveBackward, &keyboard) {
                move_delta += forward;
            }

            // Left/right (A/D)
            if keybindings.pressed(EditorAction::CameraMoveLeft, &keyboard) {
                move_delta -= right_dir;
            }
            if keybindings.pressed(EditorAction::CameraMoveRight, &keyboard) {
                move_delta += right_dir;
            }

            // Down/up (Q/E)
            if keybindings.pressed(EditorAction::CameraMoveDown, &keyboard) {
                move_delta -= Vec3::Y;
            }
            if keybindings.pressed(EditorAction::CameraMoveUp, &keyboard) {
                move_delta += Vec3::Y;
            }

            // Apply WASD movement to focus point
            if move_delta.length_squared() > 0.0 {
                let speed_mult = if keybindings.pressed(EditorAction::CameraMoveFaster, &keyboard) { 2.0 } else { 1.0 };
                orbit.focus += move_delta.normalize() * move_speed * speed_mult * delta;
            }
        }

        // Then handle mouse look (rotate view while staying in place)
        // Get updated camera position after WASD movement
        let cam_pos = orbit.focus
            + Vec3::new(
                orbit.distance * orbit.pitch.cos() * orbit.yaw.sin(),
                orbit.distance * orbit.pitch.sin(),
                orbit.distance * orbit.pitch.cos() * orbit.yaw.cos(),
            );

        for ev in mouse_motion.read() {
            // Track that mouse moved during right-click hold
            if viewport.right_click_origin.is_some() {
                viewport.right_click_moved = true;
            }
            orbit.yaw -= ev.delta.x * look_speed;
            orbit.pitch += ev.delta.y * look_speed * invert_y;
            // Clamp pitch to avoid flipping
            orbit.pitch = orbit.pitch.clamp(-1.5, 1.5);
        }

        // Recalculate focus point to keep camera in same position
        let new_dir = Vec3::new(
            orbit.pitch.cos() * orbit.yaw.sin(),
            orbit.pitch.sin(),
            orbit.pitch.cos() * orbit.yaw.cos(),
        );
        orbit.focus = cam_pos - new_dir * orbit.distance;
    }
    // Alt + Left mouse - Orbit around focus point
    else if left_pressed && alt_held {
        for ev in mouse_motion.read() {
            orbit.yaw -= ev.delta.x * orbit_speed;
            orbit.pitch += ev.delta.y * orbit_speed * invert_y;
            // Clamp pitch to avoid flipping
            orbit.pitch = orbit.pitch.clamp(-1.5, 1.5);
        }
    }
    // Left mouse drag - Forward/backward movement + horizontal look (Unreal style)
    // Vertical mouse = dolly forward/backward, Horizontal mouse = yaw rotation
    // Can be disabled (e.g., in terrain layout for brush tools)
    else if left_pressed && !alt_held && !left_click_drag_disabled {
        // Movement speed multiplier for left-click drag
        let drag_move_speed = pan_speed * 3.0;

        for ev in mouse_motion.read() {
            // 1. Apply forward/backward movement based on vertical mouse
            let forward = Vec3::new(
                orbit.yaw.sin(),
                0.0,
                orbit.yaw.cos(),
            ).normalize();

            let move_delta = forward * ev.delta.y * drag_move_speed;
            orbit.focus += move_delta;

            // 2. Get camera position after movement (before rotation)
            let cam_pos = orbit.focus
                + Vec3::new(
                    orbit.distance * orbit.pitch.cos() * orbit.yaw.sin(),
                    orbit.distance * orbit.pitch.sin(),
                    orbit.distance * orbit.pitch.cos() * orbit.yaw.cos(),
                );

            // 3. Apply yaw rotation from horizontal mouse only (no pitch change)
            orbit.yaw -= ev.delta.x * look_speed;

            // 4. Recalculate focus point to keep camera in same position after rotation
            let new_dir = Vec3::new(
                orbit.pitch.cos() * orbit.yaw.sin(),
                orbit.pitch.sin(),
                orbit.pitch.cos() * orbit.yaw.cos(),
            );
            orbit.focus = cam_pos - new_dir * orbit.distance;
        }
    }
    // Middle mouse - Orbit around focus point
    else if middle_pressed {
        for ev in mouse_motion.read() {
            orbit.yaw -= ev.delta.x * orbit_speed;
            orbit.pitch += ev.delta.y * orbit_speed * invert_y;
            // Clamp pitch to avoid flipping
            orbit.pitch = orbit.pitch.clamp(-1.5, 1.5);
        }
    } else {
        mouse_motion.clear();
    }

    // Calculate camera position from orbit parameters
    let cam_pos = orbit.focus
        + Vec3::new(
            orbit.distance * orbit.pitch.cos() * orbit.yaw.sin(),
            orbit.distance * orbit.pitch.sin(),
            orbit.distance * orbit.pitch.cos() * orbit.yaw.cos(),
        );

    transform.translation = cam_pos;
    transform.look_at(orbit.focus, Vec3::Y);
}

/// System to apply orbit state to camera transform after UI changes.
/// This runs in EguiPrimaryContextPass after editor_ui to ensure UI-triggered
/// orbit changes (view angle buttons, axis gizmo clicks, etc.) are immediately
/// reflected in the camera transform for the current frame's render.
pub fn apply_orbit_to_camera(
    orbit: Res<OrbitCameraState>,
    mut camera_query: Query<&mut Transform, With<ViewportCamera>>,
) {
    // Only run when orbit state has changed
    if !orbit.is_changed() {
        return;
    }

    let Ok(mut transform) = camera_query.single_mut() else {
        return;
    };

    let cam_pos = orbit.focus
        + Vec3::new(
            orbit.distance * orbit.pitch.cos() * orbit.yaw.sin(),
            orbit.distance * orbit.pitch.sin(),
            orbit.distance * orbit.pitch.cos() * orbit.yaw.cos(),
        );

    transform.translation = cam_pos;
    transform.look_at(orbit.focus, Vec3::Y);
}

/// System to update camera projection based on OrbitCameraState
pub fn update_camera_projection(
    orbit: Res<OrbitCameraState>,
    viewport: Res<ViewportState>,
    mut camera_query: Query<&mut Projection, With<MainCamera>>,
) {
    // Only run when projection mode has changed
    if !orbit.is_changed() && !viewport.is_changed() {
        return;
    }

    let Ok(mut projection) = camera_query.single_mut() else {
        return;
    };

    let aspect = if viewport.size[0] > 0.0 && viewport.size[1] > 0.0 {
        viewport.size[0] / viewport.size[1]
    } else {
        16.0 / 9.0
    };

    match orbit.projection_mode {
        ProjectionMode::Perspective => {
            // Switch to perspective if not already
            if !matches!(*projection, Projection::Perspective(_)) {
                *projection = Projection::Perspective(PerspectiveProjection {
                    fov: std::f32::consts::FRAC_PI_4,
                    aspect_ratio: aspect,
                    ..default()
                });
            } else if let Projection::Perspective(ref mut persp) = *projection {
                persp.aspect_ratio = aspect;
            }
        }
        ProjectionMode::Orthographic => {
            // Switch to orthographic if not already
            if !matches!(*projection, Projection::Orthographic(_)) {
                let mut ortho = OrthographicProjection::default_3d();
                // Scale based on orbit distance - larger distance = more visible area
                ortho.scale = orbit.distance / 5.0;
                *projection = Projection::Orthographic(ortho);
            } else if let Projection::Orthographic(ref mut ortho) = *projection {
                // Update orthographic scale based on orbit distance
                ortho.scale = orbit.distance / 5.0;
            }
        }
    }
}
