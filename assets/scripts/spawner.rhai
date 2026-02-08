// Spawner
//
// Periodically spawns primitive objects at the entity's position.
// Good for testing, particle-like effects, or simple enemy spawning.

fn props() {
    #{
        interval: #{ type: "float", value: 2.0, hint: "Spawn interval (seconds)" },
        primitive: #{ type: "string", value: "cube", hint: "Primitive type (cube/sphere/cylinder/capsule)" },
        spread: #{ type: "float", value: 3.0, hint: "Random spawn spread" },
        max_spawns: #{ type: "int", value: 20, hint: "Maximum number of spawns (0 = unlimited)" },
    }
}

fn on_ready() {
    last_spawn = 0.0;
    spawn_count = 0;
    start_timer("spawn", interval, true);
}

fn on_update() {
    if max_spawns > 0 && spawn_count >= max_spawns { return; }

    if timer_just_finished(timers_finished, "spawn") {
        // Random offset using sin/cos of elapsed time for variation
        let ox = sin(elapsed * 7.3) * spread;
        let oz = cos(elapsed * 11.7) * spread;

        spawn_primitive(
            primitive,
            "Spawned_" + spawn_count,
            position_x + ox,
            position_y,
            position_z + oz,
        );

        spawn_count += 1;
        log("Spawned " + primitive + " #" + spawn_count);
    }
}
