// Spin Children
//
// Rotates all child entities around the parent.
// Useful for solar systems, carousels, orbiting particles.

fn props() {
    #{
        speed: #{ type: "float", value: 30.0, hint: "Rotation speed (degrees/sec)" },
        individual_spin: #{ type: "float", value: 90.0, hint: "Each child's own spin speed" },
    }
}

fn on_update() {
    // Rotate self (parent hub)
    rotate(0.0, speed * delta, 0.0);

    // Spin each child on its own axis
    for kid in children() {
        set(kid, "rotation_y", kid.rotation_y + individual_spin * delta);
    }
}
