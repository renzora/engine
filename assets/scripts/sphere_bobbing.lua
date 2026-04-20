-- Sphere Bobbing
--
-- Gentle vertical bob for floating spheres. Multiple instances stay out of
-- phase automatically: the starting world position seeds the phase, so a
-- field of spheres bobs as a natural-looking group without per-entity setup.

function props()
    return {
        bob_height = { type = "float", value = 0.08, hint = "Vertical bob amplitude (units)" },
        bob_speed  = { type = "float", value = 0.6,  hint = "Bob frequency (Hz-ish)" },
        drift_spin = { type = "float", value = 6.0,  hint = "Slow yaw spin (deg/s, 0 to disable)" },
        _t         = { type = "float", value = 0.0,  hint = "Internal: elapsed time" },
        _base_y    = { type = "float", value = 0.0,  hint = "Internal: rest height" },
        _phase     = { type = "float", value = 0.0,  hint = "Internal: per-instance phase offset" },
        _yaw       = { type = "float", value = 0.0,  hint = "Internal: spin accumulator" },
    }
end

function on_ready()
    _t = 0.0
    _base_y = position_y
    _yaw = rotation_y
    -- Seed phase from starting position so neighboring spheres desync.
    _phase = (position_x * 0.37 + position_z * 0.91) % (2.0 * math.pi)
end

function on_update()
    _t = _t + delta

    bob = math.sin(_t * bob_speed * 2.0 * math.pi + _phase) * bob_height
    set_position(position_x, _base_y + bob, position_z)

    if drift_spin ~= 0.0 then
        _yaw = _yaw + drift_spin * delta
        set_rotation(0.0, _yaw, 0.0)
    end
end
