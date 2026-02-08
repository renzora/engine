// Patrol
//
// Moves the entity back and forth between two points.
// Useful for moving platforms, patrolling enemies, sliding doors.

fn props() {
    #{
        distance: #{ type: "float", value: 10.0, hint: "Patrol distance" },
        speed: #{ type: "float", value: 3.0, hint: "Movement speed" },
        axis_x: #{ type: "float", value: 1.0, hint: "Patrol direction X" },
        axis_y: #{ type: "float", value: 0.0, hint: "Patrol direction Y" },
        axis_z: #{ type: "float", value: 0.0, hint: "Patrol direction Z" },
        pause_time: #{ type: "float", value: 0.5, hint: "Pause at each end (seconds)" },
    }
}

fn on_ready() {
    start_x = position_x;
    start_y = position_y;
    start_z = position_z;
}

fn on_update() {
    // Ping-pong motion with pauses
    let cycle = speed / distance;
    let t = elapsed * cycle;
    // Triangle wave: goes 0->1->0 repeatedly
    let raw = t % 2.0;
    let wave = if raw <= 1.0 { raw } else { 2.0 - raw };

    // Apply pause at ends (squish the wave)
    let pause_frac = clamp(pause_time * cycle, 0.0, 0.4);
    let clamped = clamp((wave - pause_frac) / (1.0 - 2.0 * pause_frac), 0.0, 1.0);

    let offset = clamped * distance;
    set_position(
        start_x + axis_x * offset,
        start_y + axis_y * offset,
        start_z + axis_z * offset,
    );
}
