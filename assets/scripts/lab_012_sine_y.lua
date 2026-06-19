-- lab_012_sine_y: pure sine-wave vertical motion about the start height.
-- Deliberately the most minimal oscillator in the set, kept as a clean
-- reference for the sin(elapsed)*amplitude pattern around a captured base.

local base_y = 0.0

function props()
    return {
        amplitude = { value = 1.0, hint = "Wave height", tab = "Sine" },
    }
end

function on_ready()
    base_y = position_y
end

function on_update()
    set_position(position_x, base_y + math.sin(elapsed) * amplitude, position_z)
end
