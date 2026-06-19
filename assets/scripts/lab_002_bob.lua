-- lab_002_bob: vertical bobbing around a remembered base height.
-- We capture the start Y in on_ready so the bob oscillates about the
-- entity's authored position instead of drifting away from origin.

local base_y = 0.0

function props()
    return {
        amplitude = { value = 0.5, hint = "Bob height", tab = "Bob" },
        frequency = { value = 1.0, hint = "Cycles per second", tab = "Bob" },
    }
end

function on_ready()
    base_y = position_y
end

function on_update()
    local offset = math.sin(elapsed * frequency * math.pi * 2.0) * amplitude
    set_position(position_x, base_y + offset, position_z)
end
