// Light Flicker
//
// Makes a light flicker like a candle or broken fluorescent.
// Attach to an entity with a Point Light or Spot Light component.

fn props() {
    #{
        base_intensity: #{ type: "float", value: 1000.0, hint: "Base light intensity" },
        flicker_amount: #{ type: "float", value: 0.3, hint: "Flicker intensity (0-1)" },
        flicker_speed: #{ type: "float", value: 8.0, hint: "Flicker speed" },
    }
}

fn on_update() {
    // Combine multiple sine waves for organic flicker
    let noise = sin(elapsed * flicker_speed)
              + sin(elapsed * flicker_speed * 2.3) * 0.5
              + sin(elapsed * flicker_speed * 5.7) * 0.25;
    let normalized = noise / 1.75; // Normalize to roughly -1..1

    let intensity = base_intensity * (1.0 - flicker_amount * 0.5 + normalized * flicker_amount * 0.5);
    set_light_intensity(intensity);
}
