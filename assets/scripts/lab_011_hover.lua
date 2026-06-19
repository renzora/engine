-- lab_011_hover: holds the entity at a fixed hover offset above its start.
-- Eases up to the offset once on spawn (no oscillation) so it settles at a
-- raised resting height, useful for floating pickups or UI props.

local base_y = 0.0
local progress = 0.0

function props()
    return {
        offset = { value = 1.0, hint = "Hover height above start", tab = "Hover" },
        rise_speed = { value = 2.0, hint = "Rise rate", tab = "Hover" },
    }
end

function on_ready()
    base_y = position_y
end

function on_update()
    progress = clamp(progress + rise_speed * delta, 0.0, 1.0)
    set_position(position_x, base_y + offset * progress, position_z)
end
