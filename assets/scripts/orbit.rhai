// Orbit
//
// Orbits the entity around a center point at a configurable radius and speed.
// Useful for planets, moons, orbiting cameras, rotating turrets.

fn props() {
    #{
        radius: #{ type: "float", value: 5.0, hint: "Orbit radius" },
        speed: #{ type: "float", value: 1.0, hint: "Orbit speed (radians/sec)" },
        center_x: #{ type: "float", value: 0.0, hint: "Center X" },
        center_y: #{ type: "float", value: 0.0, hint: "Center Y" },
        center_z: #{ type: "float", value: 0.0, hint: "Center Z" },
        face_center: #{ type: "bool", value: true, hint: "Face toward center" },
    }
}

fn on_update() {
    let angle = elapsed * speed;
    let x = center_x + cos(angle) * radius;
    let z = center_z + sin(angle) * radius;
    set_position(x, center_y, z);

    if face_center {
        look_at(center_x, center_y, center_z);
    }
}
