// Firework Launcher
// Attach this to an entity to create a firework show.
// It uses a rising glowing sphere for the "build up" and then
// triggers the firework explosion.

fn props() {
    #{
        launch_interval: #{ type: "float", value: 3.0, hint: "Seconds between launches" },
        launch_height: #{ type: "float", value: 30.0, hint: "How high the rocket goes" },
        launch_speed: #{ type: "float", value: 20.0, hint: "How fast the rocket rises" },
        explosion_entity_name: #{ type: "string", value: "FireworkExplosion", hint: "Name of the entity with firework.particle" },
        rocket_id: #{ type: "int", value: -1 },
        target_y: #{ type: "float", value: 0.0 }
    }
}

fn on_ready() {
    start_timer_repeating("launch", launch_interval);
    rocket_id = -1;
}

fn on_update() {
    // 1. Launch a new rocket
    if timer_just_finished(timers_finished, "launch") {
        let rx = random_range(-10.0, 10.0);
        let rz = random_range(-10.0, 10.0);
        
        // Spawn a glowing sphere for the build-up
        rocket_id = spawn_sphere_at("Rocket", position_x + rx, position_y, position_z + rz);
        set_color_of(rocket_id, 1.0, 0.8, 0.2, 1.0);
        target_y = position_y + launch_height + random_range(-5.0, 5.0);
        
        play_sound("sounds/launch.ogg"); // Optional sound
    }

    // 2. Move the active rocket up
    if rocket_id != -1 && entity_exists(_found_entities, "Rocket") {
        let r = entity_id(rocket_id);
        let ry = get(r, "position_y");
        
        if ry < target_y {
            // Rise up
            translate_entity(rocket_id, 0.0, launch_speed * delta, 0.0);
            
            // Add some jitter/trail effect with debug lines if you want
            draw_line(get(r, "position_x"), ry, get(r, "position_z"), 
                      get(r, "position_x"), ry - 1.0, get(r, "position_z"));
        } else {
            // BOOM! Trigger the explosion particle at the current position
            let explosion_emitter = find_entity_by_name(_found_entities, explosion_entity_name);
            if explosion_emitter != -1 {
                particle_emit_at(explosion_emitter, get(r, "position_x"), ry, get(r, "position_z"));
                particle_burst(explosion_emitter, 800);
            }
            
            despawn_entity(rocket_id);
            rocket_id = -1;
            play_sound("sounds/explosion.ogg");
        }
    }
}

// Helper since translate() only works on self
fn translate_entity(id, x, y, z) {
    let e = entity_id(id);
    set(e, "position_x", get(e, "position_x") + x);
    set(e, "position_y", get(e, "position_y") + y);
    set(e, "position_z", get(e, "position_z") + z);
}
