-- Realistic Day / Night Cycle
--
-- Drives the scene's Sun (azimuth, elevation, colour, intensity) from a real
-- solar-position model, so the sun rises in the east, arcs across the sky at an
-- angle set by your latitude and the day of the year, and sets in the west.
-- Colour warms toward orange near the horizon (dawn / dusk) and cools to white
-- overhead; intensity tracks how high the sun is.
--
-- The engine does the rest automatically: below the horizon it fades the sun to
-- black and disables its shadows, the procedural sky reddens at the horizon and
-- darkens at night, and the night-stars system fades in. So this script only
-- has to move and tint the sun.
--
-- SETUP: attach this to the entity that has the `Sun` component (in this scene
-- that's "World Environment"). If you attach it elsewhere, set `sun_name` to the
-- name of the entity holding the Sun.
--
-- A full 24h takes `day_length` real seconds (default 120 = a 2-minute day).
-- Set `paused = true` to freeze time at the current `_time` for lighting setup.

function props()
    return {
        sun_name        = { type = "string", value = "World Environment", hint = "Name of the entity holding the Sun component" },

        -- Clock
        day_length      = { type = "float",  value = 120.0, hint = "Real seconds for one full 24h cycle" },
        start_hour      = { type = "float",  value = 8.0,   hint = "Time of day to start at (0-24)" },
        paused          = { type = "bool",   value = false, hint = "Freeze time" },

        -- Where / when on Earth (shapes the sun's arc)
        latitude        = { type = "float",  value = 40.0,  hint = "Observer latitude in degrees (-90..90)" },
        day_of_year     = { type = "float",  value = 172.0, hint = "Day of year 1-365 (172 ~ June solstice). Sets the season." },
        advance_days    = { type = "bool",   value = true,  hint = "Advance the day each cycle for slow seasonal drift" },

        -- Intensity (lux). Engine fades these to 0 below the horizon.
        peak_illuminance= { type = "float",  value = 75000.0, hint = "Midday sun brightness; scaled down as the sun lowers" },

        -- Sun colour: warm near the horizon, neutral overhead.
        warm_below      = { type = "float",  value = 18.0,  hint = "Below this elevation (deg) the sun colour warms toward the horizon tint" },
        noon_r          = { type = "float",  value = 1.0,   hint = "Overhead sun colour R" },
        noon_g          = { type = "float",  value = 0.96,  hint = "Overhead sun colour G" },
        noon_b          = { type = "float",  value = 0.90,  hint = "Overhead sun colour B" },
        dawn_r          = { type = "float",  value = 1.0,   hint = "Horizon (dawn/dusk) sun colour R" },
        dawn_g          = { type = "float",  value = 0.52,  hint = "Horizon (dawn/dusk) sun colour G" },
        dawn_b          = { type = "float",  value = 0.28,  hint = "Horizon (dawn/dusk) sun colour B" },

        -- Optional distance fog that thickens at night / dawn.
        fog             = { type = "bool",   value = false, hint = "Enable distance fog that thickens when the sun is low" },
        fog_start       = { type = "float",  value = 8.0,   hint = "Fog start distance" },
        fog_far_day     = { type = "float",  value = 320.0, hint = "Fog end distance in full daylight" },
        fog_far_night   = { type = "float",  value = 60.0,  hint = "Fog end distance at night / dawn" },

        -- Internal
        _time           = { type = "float",  value = 8.0,   hint = "Internal: current time of day (hours)" },
        _init           = { type = "bool",   value = false, hint = "Internal: clock initialised" },
    }
end

function on_ready()
    _time = start_hour
    _init = true
end

local function clamp01(v) return clamp(v, 0.0, 1.0) end
local function lerp(a, b, t) return a + (b - a) * t end

function on_update()
    if not _init then _time = start_hour; _init = true end

    -- ----- Advance the clock -----
    if not paused and day_length > 0.0 then
        _time = _time + (24.0 / day_length) * delta
        while _time >= 24.0 do
            _time = _time - 24.0
            if advance_days then
                day_of_year = day_of_year + 1.0
                if day_of_year > 365.0 then day_of_year = day_of_year - 365.0 end
            end
        end
    end

    -- ----- Solar position (standard astronomy approximation) -----
    -- Solar declination: the sun's tilt for this day of the year.
    local decl = 23.45 * math.sin(math.rad(360.0 / 365.0 * (day_of_year - 81.0)))
    -- Hour angle: 15 deg per hour away from solar noon (negative = morning).
    local hour_angle = 15.0 * (_time - 12.0)

    local lat_r  = math.rad(latitude)
    local decl_r = math.rad(decl)
    local ha_r   = math.rad(hour_angle)

    -- Elevation above the horizon.
    local sin_elev = math.sin(lat_r) * math.sin(decl_r)
                   + math.cos(lat_r) * math.cos(decl_r) * math.cos(ha_r)
    sin_elev = clamp(sin_elev, -1.0, 1.0)
    local elev = math.deg(math.asin(sin_elev))

    -- Azimuth, measured clockwise from North (N=0, E=90, S=180, W=270) to
    -- match the Sun component's convention.
    local denom = math.cos(math.rad(elev)) * math.cos(lat_r)
    local az
    if math.abs(denom) < 1e-4 then
        az = 180.0
    else
        local cos_az = (math.sin(decl_r) - math.sin(lat_r) * sin_elev) / denom
        az = math.deg(math.acos(clamp(cos_az, -1.0, 1.0)))
        if hour_angle > 0.0 then az = 360.0 - az end  -- afternoon -> west
    end

    -- ----- Colour: warm toward the horizon, neutral overhead -----
    -- 0 at/below the horizon, 1 once the sun is above `warm_below`.
    local warm = clamp01(elev / math.max(warm_below, 0.001))
    local cr = lerp(dawn_r, noon_r, warm)
    local cg = lerp(dawn_g, noon_g, warm)
    local cb = lerp(dawn_b, noon_b, warm)

    -- ----- Intensity: brightest with the sun high; ~0 near/below horizon -----
    local illum = peak_illuminance * clamp01(sin_elev)

    -- ----- Push to the Sun -----
    set_on(sun_name, "Sun.azimuth", az)
    set_on(sun_name, "Sun.elevation", elev)
    set_on(sun_name, "Sun.color.x", cr)
    set_on(sun_name, "Sun.color.y", cg)
    set_on(sun_name, "Sun.color.z", cb)
    set_on(sun_name, "Sun.illuminance", illum)

    -- ----- Optional fog that closes in when the sun is low -----
    if fog then
        local far = lerp(fog_far_night, fog_far_day, clamp01(elev / 10.0))
        set_fog(true, fog_start, far)
    end
end
