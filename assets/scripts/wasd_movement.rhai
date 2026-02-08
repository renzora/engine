// WASD Movement
//
// Simple top-down or side-scrolling movement using WASD/arrow keys.
// No rotation - just moves in the XZ plane.

fn props() {
    #{
        speed: #{ type: "float", value: 5.0, hint: "Movement speed" },
    }
}

fn on_update() {
    translate(input_x * speed * delta, 0.0, input_y * speed * delta);
}
