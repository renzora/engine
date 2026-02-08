// Proximity Trigger
//
// Monitors distance to a target entity and logs when it enters/exits range.
// Useful as a base for pickups, triggers, proximity doors.

fn props() {
    #{
        target_name: #{ type: "string", value: "Player", hint: "Entity to detect" },
        trigger_radius: #{ type: "float", value: 3.0, hint: "Detection radius" },
        one_shot: #{ type: "bool", value: false, hint: "Only trigger once" },
    }
}

fn on_ready() {
    was_inside = false;
    triggered = false;
}

fn on_update() {
    if triggered && one_shot { return; }

    let target = entity(target_name);
    if target.len() == 0 { return; }

    let dx = target.position_x - position_x;
    let dy = target.position_y - position_y;
    let dz = target.position_z - position_z;
    let dist = sqrt(dx*dx + dy*dy + dz*dz);

    let is_inside = dist < trigger_radius;

    if is_inside && !was_inside {
        log(target_name + " entered trigger zone");
        triggered = true;
    }

    if !is_inside && was_inside {
        log(target_name + " exited trigger zone");
    }

    // Debug visualization
    draw_sphere(position_x, position_y, position_z, trigger_radius);

    was_inside = is_inside;
}
