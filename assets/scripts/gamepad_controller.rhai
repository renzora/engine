// Gamepad Controller
//
// Third-person style movement using a gamepad.
// Left stick to move, right stick to look, A to jump, LB/RB sprint.

fn props() {
    #{
        move_speed: #{ type: "float", value: 8.0, hint: "Movement speed" },
        look_sensitivity: #{ type: "float", value: 120.0, hint: "Right stick look speed (degrees/sec)" },
        sprint_multiplier: #{ type: "float", value: 2.0, hint: "Sprint speed multiplier (LB/RB)" },
        deadzone: #{ type: "float", value: 0.15, hint: "Stick deadzone" },
    }
}

fn on_update() {
    // Right stick look (horizontal rotation only for third-person)
    let look_x = gamepad_right_x;
    if abs(look_x) > deadzone {
        let yaw = rotation_y - look_x * look_sensitivity * delta;
        set_rotation(rotation_x, yaw, 0.0);
    }

    // Left stick movement
    let move_x = gamepad_left_x;
    let move_y = gamepad_left_y;

    // Apply deadzone
    if abs(move_x) < deadzone { move_x = 0.0; }
    if abs(move_y) < deadzone { move_y = 0.0; }

    // Sprint with shoulder buttons
    let speed = move_speed;
    if gamepad_lb || gamepad_rb {
        speed *= sprint_multiplier;
    }

    // Move relative to facing direction
    let rad_y = deg_to_rad(rotation_y);
    let forward_x = -sin(rad_y);
    let forward_z = -cos(rad_y);
    let right_x = cos(rad_y);
    let right_z = -sin(rad_y);

    let dx = (move_x * right_x + move_y * forward_x) * speed * delta;
    let dz = (move_x * right_z + move_y * forward_z) * speed * delta;

    translate(dx, 0.0, dz);
}
