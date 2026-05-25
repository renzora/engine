-- Respawn on Fall
--
-- If the character falls below `kill_y`, snap it back to a respawn point. Useful
-- for catching the player when they walk off a ledge or fall through the world.
--
-- Attach this to the CHARACTER entity. By default the respawn point is wherever
-- the character started; tick `use_custom_spawn` to respawn at a fixed point
-- instead (e.g. a checkpoint).

function props()
    return {
        kill_y          = { type = "float", value = -15.0, hint = "Respawn when the character's Y drops below this" },
        use_custom_spawn= { type = "bool",  value = false, hint = "Respawn at spawn_x/y/z instead of the start position" },
        spawn_x         = { type = "float", value = 0.0,  hint = "Custom respawn X (if use_custom_spawn)" },
        spawn_y         = { type = "float", value = 2.0,  hint = "Custom respawn Y (if use_custom_spawn)" },
        spawn_z         = { type = "float", value = 0.0,  hint = "Custom respawn Z (if use_custom_spawn)" },

        -- Internal: captured start position
        _sx   = { type = "float", value = 0.0, hint = "Internal: start X" },
        _sy   = { type = "float", value = 0.0, hint = "Internal: start Y" },
        _sz   = { type = "float", value = 0.0, hint = "Internal: start Z" },
        _init = { type = "bool",  value = false, hint = "Internal: captured start position" },
    }
end

function on_ready()
    _sx, _sy, _sz = position_x, position_y, position_z
    _init = true
end

function on_update()
    if not _init then
        _sx, _sy, _sz, _init = position_x, position_y, position_z, true
    end

    if position_y < kill_y then
        if use_custom_spawn then
            set_position(spawn_x, spawn_y, spawn_z)
        else
            set_position(_sx, _sy, _sz)
        end
    end
end
