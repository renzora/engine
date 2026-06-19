-- lab_006_pingpong: moves back and forth along X between two extremes.
-- A triangle wave (via asin of a sine) gives constant-speed travel with sharp
-- turnarounds, unlike a plain sine which eases at the ends.

local base_x = 0.0

function props()
    return {
        distance = { value = 2.0, hint = "Half travel distance", tab = "PingPong" },
        speed = { value = 1.0, hint = "Cycles per second", tab = "PingPong" },
    }
end

function on_ready()
    base_x = position_x
end

function on_update()
    local tri = math.asin(math.sin(elapsed * speed * math.pi * 2.0)) / (math.pi * 0.5)
    set_position(base_x + tri * distance, position_y, position_z)
end
