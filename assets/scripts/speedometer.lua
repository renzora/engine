-- Drives the standalone speedometer demo.
--
-- Attach this to the SAME entity that holds the HtmlTemplate for
-- speedometer_demo.html. The template binds {{ _speed }}, so the needle and
-- centre readout animate live.

function props()
    return {
        _speed = { type = "float", value = 0.0 },
        _t     = { type = "float", value = 0.0 },
    }
end

function on_update()
    _t = _t + delta
    local s = 0.5 - 0.5 * math.cos(_t * 0.5)   -- eased 0..1 sweep
    _speed = math.floor(s * 220 + 0.5)          -- 0..220 km/h
end
