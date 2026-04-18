-- FPS Camera Controller
--
-- First-person mouse look for a camera entity.
-- Accumulates pitch and yaw to avoid drift.
--
-- Setup:
--   - Camera entity: add this script
--   - Character entity: add wasd_movement.lua
--   The WASD script reads camera_yaw from this camera.

function props()
    return {
        sensitivity = { type = "float", value = 0.15, hint = "Mouse look sensitivity" },
        invert_y    = { type = "bool",  value = false, hint = "Invert vertical axis" },
        pitch_limit = { type = "float", value = 89.0, hint = "Max look up/down angle" },
        _yaw        = { type = "float", value = 0.0, hint = "Internal: accumulated yaw" },
        _pitch      = { type = "float", value = 0.0, hint = "Internal: accumulated pitch" },
    }
end

function on_ready()
    _yaw   = rotation_y
    _pitch = rotation_x
    lock_cursor()
end

function on_update()
    -- Yaw (horizontal look)
    _yaw = _yaw - mouse_delta_x * sensitivity

    -- Pitch (vertical look)
    local dy
    if invert_y then
        dy = mouse_delta_y
    else
        dy = -mouse_delta_y
    end
    _pitch = clamp(_pitch + dy * sensitivity, -pitch_limit, pitch_limit)

    set_rotation(_pitch, _yaw, 0.0)

    -- Unlock cursor on Escape, re-lock on click
    if is_key_just_pressed("Escape") then
        unlock_cursor()
    end
    if mouse_left_just_pressed then
        lock_cursor()
    end
end
