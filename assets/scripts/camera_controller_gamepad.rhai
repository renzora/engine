// Camera Controller (Gamepad)
//
// First-person camera controller using a gamepad.
// Left stick to move, right stick to look, LB/RB to sprint.

fn props() {
    #{
        move_speed: #{ type: "float", value: 8.0, hint: "Movement speed" },
        look_sensitivity: #{ type: "float", value: 120.0, hint: "Right stick look speed (degrees/sec)" },
        sprint_multiplier: #{ type: "float", value: 2.0, hint: "Sprint speed multiplier (LB/RB)" },
        deadzone: #{ type: "float", value: 0.15, hint: "Stick deadzone" },
        _yaw: #{ type: "float", value: 0.0, hint: "Internal: accumulated yaw" },
        _pitch: #{ type: "float", value: 0.0, hint: "Internal: accumulated pitch" },
    }
}

fn on_ready() {
    _yaw = rotation_y;
    _pitch = rotation_x;
}

fn on_update() {
    // Right stick camera look - circular deadzone so diagonal input doesn't drift
    let look_x = gamepad_right_x;
    let look_y = gamepad_right_y;
    let look_len = sqrt(look_x * look_x + look_y * look_y);
    if look_len > deadzone {
        let scale = (look_len - deadzone) / (1.0 - deadzone) / look_len;
        _yaw -= look_x * scale * look_sensitivity * delta;
        _pitch = clamp(_pitch + look_y * scale * look_sensitivity * delta, -89.0, 89.0);
    }
    set_rotation(_pitch, _yaw, 0.0);

    // Left stick movement
    let move_x = gamepad_left_x;
    let move_y = gamepad_left_y;
    if abs(move_x) < deadzone { move_x = 0.0; }
    if abs(move_y) < deadzone { move_y = 0.0; }

    // Sprint with shoulder buttons
    let speed = move_speed;
    if gamepad_lb || gamepad_rb {
        speed *= sprint_multiplier;
    }

    // Move relative to facing direction
    let rad_y = deg_to_rad(_yaw);
    let forward_x = -sin(rad_y);
    let forward_z = -cos(rad_y);
    let right_x = cos(rad_y);
    let right_z = -sin(rad_y);

    let dx = (move_x * right_x + move_y * forward_x) * speed * delta;
    let dz = (move_x * right_z + move_y * forward_z) * speed * delta;

    translate(dx, 0.0, dz);
}
