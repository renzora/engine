-- Simple platform that bobs up and down
-- Attach to the platform entity

function props()
    return {
        height = { value = 2.0, hint = "How far up it moves" },
        speed = { value = 0.5, hint = "How fast it moves" },
    }
end

function on_update()
    local _speed = speed or 0.5
    local _height = height or 2.0
    local offset = math.sin(elapsed * _speed) * _height
    set_position(position_x, offset, position_z)
end
