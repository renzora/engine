-- lab_013_lerp_to_target: eases from spawn to a target point, then stops.
-- We advance a normalized t toward 1 and clamp it; once t reaches 1 the lerp
-- holds the destination exactly, so there is no jitter from overshooting.

local start = nil

function props()
    return {
        target_x = { value = 0.0, hint = "Destination X", tab = "Lerp" },
        target_y = { value = 2.0, hint = "Destination Y", tab = "Lerp" },
        target_z = { value = 0.0, hint = "Destination Z", tab = "Lerp" },
        speed = { value = 0.5, hint = "Approach rate", tab = "Lerp" },
    }
end

local t = 0.0

function on_ready()
    start = vec3(position_x, position_y, position_z)
end

function on_update()
    if t >= 1.0 then return end
    t = clamp(t + speed * delta, 0.0, 1.0)
    set_position(
        lerp(start.x, target_x, t),
        lerp(start.y, target_y, t),
        lerp(start.z, target_z, t)
    )
end
