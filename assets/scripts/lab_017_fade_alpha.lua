-- lab_017_fade_alpha: pulses material alpha to fade in and out.
-- Keeps RGB at white and only animates the 4th (alpha) channel, so the entity
-- ghosts in and out without shifting hue. Requires a blend-capable material.

function props()
    return {
        min_alpha = { value = 0.0, hint = "Most transparent", tab = "Fade" },
        max_alpha = { value = 1.0, hint = "Most opaque", tab = "Fade" },
        frequency = { value = 0.5, hint = "Cycles per second", tab = "Fade" },
    }
end

function on_update()
    local t = math.sin(elapsed * frequency * math.pi * 2.0) * 0.5 + 0.5
    set_material_color(1.0, 1.0, 1.0, lerp(min_alpha, max_alpha, t))
end
