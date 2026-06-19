-- lab_023_tilt_sway: gentle single-axis sway, like grass or a hanging sign.
-- Only the Z rotation is animated about its captured base, giving a calm
-- pendulum feel; kept low-amplitude and low-frequency on purpose.

local base_z_rot = 0.0

function props()
    return {
        amplitude = { value = 6.0, hint = "Sway degrees", tab = "Sway" },
        frequency = { value = 0.6, hint = "Sways per second", tab = "Sway" },
    }
end

function on_ready()
    base_z_rot = rotation_z
end

function on_update()
    local rz = base_z_rot + math.sin(elapsed * frequency * math.pi * 2.0) * amplitude
    set_rotation(rotation_x, rotation_y, rz)
end
