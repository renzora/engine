// Look At Target
//
// Continuously faces a named entity. Useful for billboards, turrets,
// NPCs that track the player, security cameras.

fn props() {
    #{
        target_name: #{ type: "string", value: "Player", hint: "Name of entity to look at" },
        speed: #{ type: "float", value: 0.0, hint: "Rotation speed (0 = instant)" },
    }
}

fn on_update() {
    let target = entity(target_name);
    if target.len() == 0 { return; }

    look_at(target.position_x, target.position_y, target.position_z);
}
