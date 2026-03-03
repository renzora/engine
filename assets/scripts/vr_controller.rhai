// VR Controller
//
// VR locomotion using head-relative movement and snap/smooth turning.
// Left thumbstick moves, right thumbstick turns.
// Attach to any camera entity — no VrCameraRig or Play mode needed.

fn props() {
    #{
        move_speed: #{ type: "float", value: 2.0, hint: "Movement speed (m/s)" },
        snap_angle: #{ type: "float", value: 45.0, hint: "Snap turn degrees (0 = smooth turn)" },
        turn_speed: #{ type: "float", value: 120.0, hint: "Smooth turn speed (degrees/sec, used when snap_angle is 0)" },
        deadzone: #{ type: "float", value: 0.2, hint: "Thumbstick deadzone" },
        _yaw: #{ type: "float", value: 0.0, hint: "Internal: accumulated yaw" },
        _snap_cooldown: #{ type: "float", value: 0.0, hint: "Internal: snap turn cooldown timer" },
    }
}

fn on_ready() {
    _yaw = rotation_y;
}

fn on_update() {
    // Only run when VR is active
    if !is_vr_active() { return; }

    // ---- Right thumbstick: turning ----
    let stick = get_vr_thumbstick("right");
    let turn_x = stick[0];

    if snap_angle > 0.0 {
        // Snap turning with cooldown
        _snap_cooldown -= delta;
        if _snap_cooldown < 0.0 { _snap_cooldown = 0.0; }

        if abs(turn_x) > deadzone && _snap_cooldown <= 0.0 {
            if turn_x > 0.0 {
                _yaw -= snap_angle;
            } else {
                _yaw += snap_angle;
            }
            _snap_cooldown = 0.3;
        }
    } else {
        // Smooth turning
        if abs(turn_x) > deadzone {
            _yaw -= turn_x * turn_speed * delta;
        }
    }
    set_rotation(0.0, _yaw, 0.0);

    // ---- Left thumbstick: locomotion ----
    let move_stick = get_vr_thumbstick("left");
    let move_x = move_stick[0];
    let move_y = move_stick[1];

    // Apply deadzone
    if abs(move_x) < deadzone { move_x = 0.0; }
    if abs(move_y) < deadzone { move_y = 0.0; }

    if move_x == 0.0 && move_y == 0.0 { return; }

    // Move relative to current yaw (head-relative on XZ plane)
    let rad_y = deg_to_rad(_yaw);
    let forward_x = -sin(rad_y);
    let forward_z = -cos(rad_y);
    let right_x = cos(rad_y);
    let right_z = -sin(rad_y);

    let dx = (move_x * right_x + move_y * forward_x) * move_speed * delta;
    let dz = (move_x * right_z + move_y * forward_z) * move_speed * delta;

    translate(dx, 0.0, dz);
}
