// Orbit
//
// Orbits this entity around a target entity or a fixed world-space point.
//
// To orbit a named entity (e.g. a cube called "Cube"):
//   - Set `target` to the entity's name in the inspector
//   - The camera will orbit that entity's current position each frame
//
// To orbit a fixed point:
//   - Leave `target` empty and set center_x / center_y / center_z manually

fn props() {
    #{
        target:     #{ type: "string", value: "",   hint: "Entity name to orbit (leave empty to use center_x/y/z)" },
        radius:     #{ type: "float",  value: 5.0,  hint: "Orbit radius" },
        height:     #{ type: "float",  value: 2.0,  hint: "Height above the target" },
        speed:      #{ type: "float",  value: 1.0,  hint: "Orbit speed (radians/sec)" },
        center_x:   #{ type: "float",  value: 0.0,  hint: "Fixed center X (used when target is empty)" },
        center_y:   #{ type: "float",  value: 0.0,  hint: "Fixed center Y (used when target is empty)" },
        center_z:   #{ type: "float",  value: 0.0,  hint: "Fixed center Z (used when target is empty)" },
        face_center: #{ type: "bool",  value: true, hint: "Always look toward the center" },
    }
}

fn on_update() {
    let cx = center_x;
    let cy = center_y;
    let cz = center_z;

    if target != "" {
        let t = entity(target);
        if t != () {
            cx = t.position_x;
            cy = t.position_y;
            cz = t.position_z;
        }
    }

    let angle = elapsed * speed;
    set_position(cx + cos(angle) * radius, cy + height, cz + sin(angle) * radius);

    if face_center {
        look_at(cx, cy, cz);
    }
}
