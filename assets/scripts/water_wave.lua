-- Water Wave
--
-- Gentle whole-plane motion for a water surface. This animates the plane's
-- transform (subtle bob + faint tilt), not the mesh vertices — so it reads
-- as a slow swell rather than rippling waves. Pair with a scrolling normal
-- map in the material for surface detail.

function props()
    return {
        swell_height = { type = "float", value = 0.03, hint = "Vertical swell amplitude (units)" },
        swell_speed  = { type = "float", value = 0.25, hint = "Swell frequency (Hz-ish)" },
        tilt_angle   = { type = "float", value = 0.4,  hint = "Subtle tilt amplitude (deg)" },
        tilt_speed   = { type = "float", value = 0.18, hint = "Tilt frequency" },
        _t           = { type = "float", value = 0.0,  hint = "Internal: elapsed time" },
        _base_x      = { type = "float", value = 0.0,  hint = "Internal: rest position x" },
        _base_y      = { type = "float", value = 0.0,  hint = "Internal: rest position y" },
        _base_z      = { type = "float", value = 0.0,  hint = "Internal: rest position z" },
        _base_yaw    = { type = "float", value = 0.0,  hint = "Internal: rest yaw (deg)" },
    }
end

function on_ready()
    _t = 0.0
    _base_x = position_x
    _base_y = position_y
    _base_z = position_z
    _base_yaw = rotation_y
end

function on_update()
    _t = _t + delta

    swell = math.sin(_t * swell_speed * 2.0 * math.pi) * swell_height
    tilt_x = math.sin(_t * tilt_speed * 2.0 * math.pi)        * tilt_angle
    tilt_z = math.sin(_t * tilt_speed * 2.0 * math.pi + 1.7)  * tilt_angle

    set_position(_base_x, _base_y + swell, _base_z)
    set_rotation(tilt_x, _base_yaw, tilt_z)
end
