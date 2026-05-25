-- Interactable Door
--
-- Attach this to the DOOR entity. When the player is within `open_distance`, a
-- UI prompt appears; pressing the interact button (E / gamepad R1) toggles the
-- door open/closed by swinging it `open_angle` degrees about its Y axis.
--
-- IMPORTANT — pivot: a door swings about its hinge, so the door mesh's ORIGIN
-- must sit at the hinge edge, not the centre. If it spins about the middle,
-- re-pivot the mesh (or parent it to an empty placed at the hinge and put this
-- script on that parent).
--
-- UI prompt: create a UI Text widget in your scene and put its name in
-- `prompt_name` (default "DoorPrompt"). Set `prompt_name = ""` to skip the UI
-- and just use proximity + key.

function props()
    return {
        player_name   = { type = "string", value = "character.glb", hint = "Name of the player entity to measure distance to" },
        prompt_name   = { type = "string", value = "DoorPrompt",    hint = "Name of the UI widget to show as a prompt ('' = none)" },
        open_text     = { type = "string", value = "Open Door  [E]", hint = "Prompt text when the door is closed" },
        close_text    = { type = "string", value = "Close Door [E]", hint = "Prompt text when the door is open" },
        open_distance = { type = "float",  value = 2.5,  hint = "How close the player must be (world units)" },
        open_angle    = { type = "float",  value = 90.0, hint = "Swing angle in degrees" },
        open_speed    = { type = "float",  value = 220.0, hint = "Swing speed (deg/sec)" },
        interact_key  = { type = "string", value = "KeyE", hint = "Keyboard key to interact" },
        gamepad_button= { type = "string", value = "r1",   hint = "Gamepad button: r1 l1 r3 l3 north south east west dpad_up dpad_down dpad_left dpad_right start select" },

        -- Internal state
        _brx  = { type = "float", value = 0.0, hint = "Internal: base rotation X" },
        _bry  = { type = "float", value = 0.0, hint = "Internal: base (closed) yaw" },
        _brz  = { type = "float", value = 0.0, hint = "Internal: base rotation Z" },
        _yaw  = { type = "float", value = 0.0, hint = "Internal: current yaw" },
        _open = { type = "bool",  value = false, hint = "Internal: target open state" },
        _prev_btn  = { type = "bool", value = false, hint = "Internal: interact button held last frame" },
        _was_near  = { type = "bool", value = false, hint = "Internal: player was near last frame" },
        _init = { type = "bool",  value = false, hint = "Internal: captured base rotation" },
    }
end

function on_ready()
    _brx, _bry, _brz = rotation_x, rotation_y, rotation_z
    _yaw = rotation_y
    _open = false
    _prev_btn = false
    _was_near = false
    _init = true
    if prompt_name ~= "" then action("ui_hide", { name = prompt_name }) end
end

-- Read the currently-configured gamepad interact button (level, not edge).
local function gamepad_interact_down()
    local b = gamepad_button
    if b == "r1" then return gamepad_r1
    elseif b == "l1" then return gamepad_l1
    elseif b == "r3" then return gamepad_r3
    elseif b == "l3" then return gamepad_l3
    elseif b == "north" then return gamepad_north
    elseif b == "south" then return gamepad_south
    elseif b == "east" then return gamepad_east
    elseif b == "west" then return gamepad_west
    elseif b == "dpad_up" then return gamepad_dpad_up
    elseif b == "dpad_down" then return gamepad_dpad_down
    elseif b == "dpad_left" then return gamepad_dpad_left
    elseif b == "dpad_right" then return gamepad_dpad_right
    elseif b == "start" then return gamepad_start
    elseif b == "select" then return gamepad_select
    end
    return false
end

-- Push the current prompt text for the door's state.
local function update_prompt()
    if prompt_name ~= "" then
        action("ui_set_text", { name = prompt_name, text = _open and close_text or open_text })
    end
end

function on_update()
    if not _init then
        _brx, _bry, _brz = rotation_x, rotation_y, rotation_z
        _yaw, _init = rotation_y, true
    end

    -- Distance to the player (horizontal).
    local px = get_on(player_name, "Transform.translation.x")
    local pz = get_on(player_name, "Transform.translation.z")
    local near = false
    if type(px) == "number" and type(pz) == "number" then
        local dx = px - position_x
        local dz = pz - position_z
        near = (dx * dx + dz * dz) < (open_distance * open_distance)
    end

    -- Interact = keyboard key edge OR the configured gamepad button's edge.
    local btn = gamepad_interact_down()
    local interact = is_key_just_pressed(interact_key) or (btn and not _prev_btn)
    _prev_btn = btn

    if near then
        if not _was_near and prompt_name ~= "" then
            action("ui_show", { name = prompt_name })
            update_prompt()
        end
        if interact then
            _open = not _open
            update_prompt()
        end
    elseif _was_near and prompt_name ~= "" then
        action("ui_hide", { name = prompt_name })
    end
    _was_near = near

    -- Swing toward the target angle.
    local target = _bry + (_open and open_angle or 0.0)
    local diff = target - _yaw
    local step = open_speed * delta
    if math.abs(diff) <= step then
        _yaw = target
    else
        _yaw = _yaw + (diff > 0.0 and step or -step)
    end
    set_rotation(_brx, _yaw, _brz)
end
