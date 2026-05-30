-- Animated data source for the vector component dashboard.
--
-- Attach this to the SAME entity that holds the HtmlTemplate for
-- vector_lib_demo.html. The template binds these variables with {{ _name }},
-- so the speedometers / gauges / charts animate live.

function props()
    return {
        _speed = { type = "float",  value = 0.0 },
        _rpm   = { type = "float",  value = 0.0 },
        _temp  = { type = "float",  value = 0.0 },
        _fuel  = { type = "float",  value = 0.0 },
        _bars  = { type = "string", value = "" },
        _line  = { type = "string", value = "" },
        _wave  = { type = "string", value = "" },
        _t     = { type = "float",  value = 0.0 },
    }
end

-- Lua state persists across on_update calls (same VM), so we can keep a rolling
-- history for the line chart.
local line_hist = {}

function on_update()
    _t = _t + delta
    local s = 0.5 - 0.5 * math.cos(_t * 0.5)  -- eased 0..1 sweep

    _speed = math.floor(20 + s * 210 + 0.5)   -- 20..230 km/h
    _rpm   = 1.0 + s * 7.0                     -- 1..8 (x1000)
    _temp  = math.floor(55 + 35 * math.sin(_t * 0.4) + 0.5)
    _fuel  = 0.5 + 0.45 * math.sin(_t * 0.2)   -- 0..1

    -- 8 phase-shifted bars.
    local bars = {}
    for i = 1, 8 do
        bars[i] = string.format("%.3f", 0.2 + 0.7 * math.abs(math.sin(_t * 1.2 + i * 0.6)))
    end
    _bars = table.concat(bars, ",")

    -- Rolling line history (newest pushed on the right).
    line_hist[#line_hist + 1] = 0.5 + 0.4 * math.sin(_t * 1.5)
    if #line_hist > 24 then table.remove(line_hist, 1) end
    local lp = {}
    for i = 1, #line_hist do lp[i] = string.format("%.3f", line_hist[i]) end
    _line = table.concat(lp, ",")

    -- 32-sample scrolling waveform.
    local wf = {}
    for i = 1, 32 do
        wf[i] = string.format("%.3f", math.abs(math.sin(_t * 4 + i * 0.5)) * (0.3 + 0.6 * s))
    end
    _wave = table.concat(wf, ",")
end
