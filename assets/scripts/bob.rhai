// Bob
//
// Oscillates the entity up and down with a sine wave.
// Useful for floating items, hovering platforms, idle animations.

fn props() {
    #{
        amplitude: #{ type: "float", value: 0.5, hint: "Height of oscillation" },
        speed: #{ type: "float", value: 2.0, hint: "Oscillation speed" },
        offset: #{ type: "float", value: 0.0, hint: "Phase offset (0-6.28)" },
        base_y: #{ type: "float", value: 0.0, hint: "Starting Y position (set automatically)" },
    }
}

fn on_ready() {
    // Store the starting Y position
    base_y = position_y;
}

fn on_update() {
    let y = base_y + sin(elapsed * speed + offset) * amplitude;
    set_position(position_x, y, position_z);
}
