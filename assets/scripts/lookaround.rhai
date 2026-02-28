// Camera Controller (Mouse)
//
// First-person mouse look for a camera entity.
// Move the mouse to look around. Accumulates pitch and yaw to avoid drift.
//
// Typical setup:
//   - Camera entity: add this script
//   - Character entity: add wasd_movement.rhai
//   The WASD script automatically reads camera_yaw from this camera.

fn props() {
    #{
        sensitivity: #{ type: "float", value: 0.15, hint: "Mouse look sensitivity" },
        invert_y:    #{ type: "bool",  value: false, hint: "Invert vertical (pitch) axis" },
        _yaw:        #{ type: "float", value: 0.0,   hint: "Internal: accumulated yaw" },
        _pitch:      #{ type: "float", value: 0.0,   hint: "Internal: accumulated pitch" },
    }
}

fn on_ready() {
    _yaw   = rotation_y;
    _pitch = rotation_x;
}

fn on_update() {
    _yaw -= mouse_delta_x * sensitivity;

    let dy = if invert_y { mouse_delta_y } else { -mouse_delta_y };
    _pitch = clamp(_pitch + dy * sensitivity, -89.0, 89.0);

    set_rotation(_pitch, _yaw, 0.0);
}
