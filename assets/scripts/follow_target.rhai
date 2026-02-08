// Follow Target
//
// Smoothly follows a named target entity with configurable offset and smoothing.
// Useful for cameras, companions, UI elements that track objects.

fn props() {
    #{
        target_name: #{ type: "string", value: "Player", hint: "Name of entity to follow" },
        offset_x: #{ type: "float", value: 0.0, hint: "Offset X from target" },
        offset_y: #{ type: "float", value: 5.0, hint: "Offset Y from target" },
        offset_z: #{ type: "float", value: -10.0, hint: "Offset Z from target" },
        smoothing: #{ type: "float", value: 5.0, hint: "Follow smoothing (higher = faster)" },
        look_at_target: #{ type: "bool", value: true, hint: "Face the target" },
    }
}

fn on_update() {
    let target = entity(target_name);
    if target.len() == 0 { return; }

    let goal_x = target.position_x + offset_x;
    let goal_y = target.position_y + offset_y;
    let goal_z = target.position_z + offset_z;

    let t = clamp(smoothing * delta, 0.0, 1.0);
    let new_x = lerp(position_x, goal_x, t);
    let new_y = lerp(position_y, goal_y, t);
    let new_z = lerp(position_z, goal_z, t);

    set_position(new_x, new_y, new_z);

    if look_at_target {
        look_at(target.position_x, target.position_y, target.position_z);
    }
}
