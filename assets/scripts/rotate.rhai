// Rotate
//
// Continuously rotates the entity around a configurable axis.
// Great for spinning objects, fans, windmills, etc.

fn props() {
    #{
        speed: #{ type: "float", value: 45.0, hint: "Rotation speed (degrees/sec)" },
        axis_x: #{ type: "float", value: 0.0, hint: "Rotation axis X" },
        axis_y: #{ type: "float", value: 1.0, hint: "Rotation axis Y" },
        axis_z: #{ type: "float", value: 0.0, hint: "Rotation axis Z" },
    }
}

fn on_update() {
    rotate(axis_x * speed * delta, axis_y * speed * delta, axis_z * speed * delta);
}
