-- lab_010_wobble: tilts on X and Z with two slightly detuned sines.
-- Using different frequencies per axis keeps the wobble from collapsing into a
-- single flat plane of motion, giving an organic, never-quite-repeating sway.

local base_x_rot = 0.0
local base_z_rot = 0.0

function props()
    return {
        amplitude = { value = 8.0, hint = "Max tilt degrees", tab = "Wobble" },
    }
end

function on_ready()
    base_x_rot = rotation_x
    base_z_rot = rotation_z
end

function on_update()
    local rx = base_x_rot + math.sin(elapsed * 1.3) * amplitude
    local rz = base_z_rot + math.sin(elapsed * 1.7) * amplitude
    set_rotation(rx, rotation_y, rz)
end
