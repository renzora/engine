-- Boat Rocking
--
-- Gentle floating motion: vertical bob plus pitch and roll.
-- Uses three sine waves with different frequencies so the motion never looks
-- perfectly periodic. Attach to the boat root entity; it offsets from the
-- transform that was set when the script became ready.

function props()
    return {
        bob_height    = { type = "float", value = 0.04, hint = "Vertical bob amplitude (units)" },
        bob_speed     = { type = "float", value = 0.5,  hint = "Vertical bob frequency (Hz-ish)" },
        roll_angle    = { type = "float", value = 1.2,  hint = "Side-to-side roll amplitude (deg)" },
        roll_speed    = { type = "float", value = 0.35, hint = "Roll frequency" },
        pitch_angle   = { type = "float", value = 0.7,  hint = "Fore/aft pitch amplitude (deg)" },
        pitch_speed   = { type = "float", value = 0.55, hint = "Pitch frequency" },
        _t            = { type = "float", value = 0.0,  hint = "Internal: elapsed time" },
        _base_x       = { type = "float", value = 0.0,  hint = "Internal: rest position x" },
        _base_y       = { type = "float", value = 0.0,  hint = "Internal: rest position y" },
        _base_z       = { type = "float", value = 0.0,  hint = "Internal: rest position z" },
        _base_yaw     = { type = "float", value = 0.0,  hint = "Internal: rest yaw (deg)" },
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

    -- Vertical bob
    bob = math.sin(_t * bob_speed * 2.0 * math.pi) * bob_height

    -- Roll (around forward axis) and pitch (around side axis)
    -- Phase-shift so they don't peak together.
    roll  = math.sin(_t * roll_speed  * 2.0 * math.pi)        * roll_angle
    pitch = math.sin(_t * pitch_speed * 2.0 * math.pi + 1.3)  * pitch_angle

    set_position(_base_x, _base_y + bob, _base_z)
    set_rotation(pitch, _base_yaw, roll)
end
