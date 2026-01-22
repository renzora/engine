use bevy::prelude::*;
use bevy::input::mouse::{MouseMotion, MouseWheel};

use crate::core::{EditorState, EditorEntity, ViewportCamera, KeyBindings, EditorAction};

pub fn camera_controller(
    mut editor_state: ResMut<EditorState>,
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut scroll_events: MessageReader<MouseWheel>,
    mut camera_query: Query<&mut Transform, With<ViewportCamera>>,
    entity_query: Query<&Transform, (With<EditorEntity>, Without<ViewportCamera>)>,
) {
    let Ok(mut transform) = camera_query.single_mut() else {
        return;
    };

    // Focus on selected entity (works even when not hovering viewport)
    if keybindings.just_pressed(EditorAction::FocusSelected, &keyboard) {
        if let Some(selected) = editor_state.selected_entity {
            if let Ok(target_transform) = entity_query.get(selected) {
                editor_state.orbit_focus = target_transform.translation;
                // Optionally adjust distance based on object bounds (simplified to fixed distance)
                editor_state.orbit_distance = 5.0;
            }
        }
    }

    if !editor_state.viewport_hovered {
        mouse_motion.clear();
        scroll_events.clear();
        return;
    }

    let orbit_speed = 0.005;
    let pan_speed = 0.01;
    let zoom_speed = 1.0;
    let move_speed = editor_state.camera_move_speed;
    let delta = time.delta_secs();

    let middle_pressed = mouse_button.pressed(MouseButton::Middle);
    let right_pressed = mouse_button.pressed(MouseButton::Right);
    let middle_just_pressed = mouse_button.just_pressed(MouseButton::Middle);
    let right_just_pressed = mouse_button.just_pressed(MouseButton::Right);
    let shift_held = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

    // Clear accumulated events on first frame of button press to prevent jump
    if middle_just_pressed || right_just_pressed {
        mouse_motion.clear();
        return;
    }

    // WASD navigation - move the orbit focus point
    let mut move_delta = Vec3::ZERO;

    // Get camera forward/right on XZ plane
    let forward = Vec3::new(
        editor_state.orbit_yaw.sin(),
        0.0,
        editor_state.orbit_yaw.cos(),
    ).normalize();
    let right_dir = Vec3::new(forward.z, 0.0, -forward.x);

    // Forward/backward
    if keybindings.pressed(EditorAction::CameraMoveForward, &keyboard) {
        move_delta -= forward;
    }
    if keybindings.pressed(EditorAction::CameraMoveBackward, &keyboard) {
        move_delta += forward;
    }

    // Left/right
    if keybindings.pressed(EditorAction::CameraMoveLeft, &keyboard) {
        move_delta -= right_dir;
    }
    if keybindings.pressed(EditorAction::CameraMoveRight, &keyboard) {
        move_delta += right_dir;
    }

    // Down/up
    if keybindings.pressed(EditorAction::CameraMoveDown, &keyboard) {
        move_delta -= Vec3::Y;
    }
    if keybindings.pressed(EditorAction::CameraMoveUp, &keyboard) {
        move_delta += Vec3::Y;
    }

    // Apply movement (faster with modifier)
    if move_delta.length_squared() > 0.0 {
        let speed_mult = if keybindings.pressed(EditorAction::CameraMoveFaster, &keyboard) { 2.0 } else { 1.0 };
        editor_state.orbit_focus += move_delta.normalize() * move_speed * speed_mult * delta;
    }

    // Scroll wheel - zoom
    for ev in scroll_events.read() {
        editor_state.orbit_distance -= ev.y * zoom_speed;
        editor_state.orbit_distance = editor_state.orbit_distance.clamp(1.0, 100.0);
    }

    // Middle mouse + Shift OR Right mouse - pan
    if (middle_pressed && shift_held) || right_pressed {
        for ev in mouse_motion.read() {
            let right = transform.right();
            let up = transform.up();
            let pan_delta = -*right * ev.delta.x * pan_speed * editor_state.orbit_distance * 0.1
                + *up * ev.delta.y * pan_speed * editor_state.orbit_distance * 0.1;
            editor_state.orbit_focus += pan_delta;
        }
    }
    // Middle mouse - orbit
    else if middle_pressed {
        for ev in mouse_motion.read() {
            editor_state.orbit_yaw -= ev.delta.x * orbit_speed;
            editor_state.orbit_pitch += ev.delta.y * orbit_speed;
            // Clamp pitch to avoid flipping
            editor_state.orbit_pitch = editor_state.orbit_pitch.clamp(-1.5, 1.5);
        }
    } else {
        mouse_motion.clear();
    }

    // Calculate camera position from orbit parameters
    let cam_pos = editor_state.orbit_focus
        + Vec3::new(
            editor_state.orbit_distance * editor_state.orbit_pitch.cos() * editor_state.orbit_yaw.sin(),
            editor_state.orbit_distance * editor_state.orbit_pitch.sin(),
            editor_state.orbit_distance * editor_state.orbit_pitch.cos() * editor_state.orbit_yaw.cos(),
        );

    transform.translation = cam_pos;
    transform.look_at(editor_state.orbit_focus, Vec3::Y);
}
