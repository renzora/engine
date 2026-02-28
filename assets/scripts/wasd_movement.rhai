// WASD Movement
//
// Camera-relative movement using WASD/arrow keys.
// Moves forward/back/left/right relative to the active camera's facing direction.
// Pair with camera_controller_mouse.rhai on a camera entity for full character + look setup.

fn props() {
    #{
        speed: #{ type: "float", value: 5.0, hint: "Movement speed" },
    }
}

fn on_update() {
    let rad = deg_to_rad(camera_yaw);
    let forward_x = -sin(rad);
    let forward_z = -cos(rad);
    let right_x = cos(rad);
    let right_z = -sin(rad);

    let dx = (input_x * right_x + input_y * forward_x) * speed * delta;
    let dz = (input_x * right_z + input_y * forward_z) * speed * delta;

    translate(dx, 0.0, dz);
}
