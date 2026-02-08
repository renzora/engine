// Color Cycle
//
// Cycles the entity's material color through a smooth rainbow or between two colors.
// Attach to any entity with a mesh and material.

fn props() {
    #{
        speed: #{ type: "float", value: 1.0, hint: "Cycle speed" },
        saturation: #{ type: "float", value: 0.8, hint: "Color saturation (0-1)" },
        brightness: #{ type: "float", value: 1.0, hint: "Color brightness (0-1)" },
    }
}

fn on_update() {
    // HSV to RGB conversion (hue cycles over time)
    let hue = (elapsed * speed) % 1.0;
    let h = hue * 6.0;
    let c = brightness * saturation;
    let x = c * (1.0 - abs(h % 2.0 - 1.0));
    let m = brightness - c;

    let r = 0.0;
    let g = 0.0;
    let b = 0.0;

    if h < 1.0 { r = c; g = x; }
    else if h < 2.0 { r = x; g = c; }
    else if h < 3.0 { g = c; b = x; }
    else if h < 4.0 { g = x; b = c; }
    else if h < 5.0 { r = x; b = c; }
    else { r = c; b = x; }

    set_material_color(r + m, g + m, b + m, 1.0);
}
