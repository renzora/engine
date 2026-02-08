// Debug Info
//
// Prints entity state to the console at a configurable interval.
// Useful for debugging transforms, variables, and entity data.

fn props() {
    #{
        interval: #{ type: "float", value: 1.0, hint: "Print interval (seconds, 0 = every frame)" },
        show_position: #{ type: "bool", value: true, hint: "Show position" },
        show_rotation: #{ type: "bool", value: false, hint: "Show rotation" },
        show_scale: #{ type: "bool", value: false, hint: "Show scale" },
    }
}

fn on_ready() {
    last_print = 0.0;
}

fn on_update() {
    if interval > 0.0 && elapsed - last_print < interval { return; }
    last_print = elapsed;

    let info = "[" + self_entity_name + "] ";

    if show_position {
        info += "pos=(" + round(position_x * 100.0) / 100.0
             + ", " + round(position_y * 100.0) / 100.0
             + ", " + round(position_z * 100.0) / 100.0 + ") ";
    }

    if show_rotation {
        info += "rot=(" + round(rotation_x * 10.0) / 10.0
             + ", " + round(rotation_y * 10.0) / 10.0
             + ", " + round(rotation_z * 10.0) / 10.0 + ") ";
    }

    if show_scale {
        info += "scale=(" + round(scale_x * 100.0) / 100.0
             + ", " + round(scale_y * 100.0) / 100.0
             + ", " + round(scale_z * 100.0) / 100.0 + ") ";
    }

    log(info);
}
