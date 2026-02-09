// FPS Controller
//
// First-person movement and mouse look controller.
// WASD to move, mouse to look. Requires play mode with mouse capture.

fn props() {
    #{
        move_speed: #{ type: "float", value: 8.0, hint: "Movement speed" },
        mouse_sensitivity: #{ type: "float", value: 0.15, hint: "Mouse look sensitivity" },
        sprint_multiplier: #{ type: "float", value: 2.0, hint: "Sprint speed multiplier" },
    }
}

fn on_update() {
    // Mouse look
    log("Mouse dX: " + mouse_delta_x + "  dY: " + mouse_delta_y);
    let yaw = rotation_y - mouse_delta_x * mouse_sensitivity;
    let pitch = clamp(rotation_x - mouse_delta_y * mouse_sensitivity, -89.0, 89.0);
    set_rotation(pitch, yaw, 0.0);

    // Debug keyboard input
    log("Key W: " + _keys_pressed.is_key_pressed("KeyW") + "  A: " + _keys_pressed.is_key_pressed("KeyA") + "  S: " + _keys_pressed.is_key_pressed("KeyS") + "  D: " + _keys_pressed.is_key_pressed("KeyD"));

    // Movement relative to facing direction
    log("Input X: " + input_x + "  Y: " + input_y);
    let speed = move_speed;
    if _keys_pressed.is_key_pressed("ShiftLeft") {
        speed *= sprint_multiplier;
    }

    // Forward/back along facing direction
    let rad_y = deg_to_rad(yaw);
    let forward_x = -sin(rad_y);
    let forward_z = -cos(rad_y);
    let right_x = cos(rad_y);
    let right_z = -sin(rad_y);

    let move_x = (input_x * right_x + input_y * forward_x) * speed * delta;
    let move_z = (input_x * right_z + input_y * forward_z) * speed * delta;

    translate(move_x, 0.0, move_z);
}
