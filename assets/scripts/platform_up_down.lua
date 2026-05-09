-- Platform Up/Down
--
-- Moving platform that rises by travel_height, pauses at the top, falls back
-- to its rest position, pauses at the bottom, and repeats. Smoothstep easing
-- on the travel segments keeps starts and stops gentle so a rider isn't
-- launched. Attach to the platform root; rest position is captured on ready.

function props()
    return {
        travel_height = { type = "float", value = 3.0,  hint = "How far up the platform travels (units)" },
        travel_time   = { type = "float", value = 1.5,  hint = "Seconds to move from bottom to top (and top to bottom)" },
        dwell_top     = { type = "float", value = 0.75, hint = "Seconds to wait at the top" },
        dwell_bottom  = { type = "float", value = 0.75, hint = "Seconds to wait at the bottom" },
        start_delay   = { type = "float", value = 0.0,  hint = "Initial delay before first rise (seconds)" },
        _t            = { type = "float", value = 0.0,  hint = "Internal: elapsed time" },
        _base_x       = { type = "float", value = 0.0,  hint = "Internal: rest position x" },
        _base_y       = { type = "float", value = 0.0,  hint = "Internal: rest position y" },
        _base_z       = { type = "float", value = 0.0,  hint = "Internal: rest position z" },
    }
end

function on_ready()
    _t = -start_delay
    _base_x = position_x
    _base_y = position_y
    _base_z = position_z
end

function on_update()
    _t = _t + delta

    cycle = travel_time * 2.0 + dwell_top + dwell_bottom
    -- Lua 5.x has no integer modulo for floats; fmod works for our positive cycle.
    local phase = math.fmod(math.max(_t, 0.0), cycle)

    local k
    if phase < travel_time then
        k = phase / travel_time                     -- rising
    elseif phase < travel_time + dwell_top then
        k = 1.0                                     -- holding at top
    elseif phase < travel_time * 2.0 + dwell_top then
        k = 1.0 - (phase - travel_time - dwell_top) / travel_time  -- falling
    else
        k = 0.0                                     -- holding at bottom
    end

    -- Smoothstep so the platform eases in and out of motion.
    local s = k * k * (3.0 - 2.0 * k)
    set_position(_base_x, _base_y + s * travel_height, _base_z)
end
