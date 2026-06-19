-- lab_004_color_pulse: pulses the material brightness between two greys.
-- sin() returns -1..1, so we remap to 0..1 with *0.5+0.5 before lerping the
-- channel value; otherwise half the cycle would clamp to black.

function props()
    return {
        low = { value = 0.1, hint = "Darkest value", tab = "Color" },
        high = { value = 0.9, hint = "Brightest value", tab = "Color" },
        frequency = { value = 1.0, hint = "Cycles per second", tab = "Color" },
    }
end

function on_update()
    local t = math.sin(elapsed * frequency * math.pi * 2.0) * 0.5 + 0.5
    local v = lerp(low, high, t)
    set_material_color(v, v, v)
end
