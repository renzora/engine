-- Wheel Steering
--
-- Rotates the front wheels based on the left gamepad stick X axis.
-- Attach this to the CAR root entity. The wheel entities must be DIRECT
-- children of the script entity — `set_child_rotation` matches by exact
-- name on the immediate child list, not deep descendants.
--
-- If your wheels survived the import flatten with names like
-- `wheel_frontleft.Mesh` instead of plain `wheel_frontleft`, just update
-- the `front_left` / `front_right` props in the inspector.

function props()
    return {
        front_left    = { type = "string", value = "wheel_frontleft",  hint = "Name of the left front wheel entity" },
        front_right   = { type = "string", value = "wheel_frontright", hint = "Name of the right front wheel entity" },
        max_angle_deg = { type = "float",  value = 30.0, hint = "Max wheel turn at full stick deflection (degrees)" },
        deadzone      = { type = "float",  value = 0.10, hint = "Stick magnitude under this snaps to zero" },
        smoothing     = { type = "float",  value = 12.0, hint = "Higher = wheels snap to target angle faster" },
        invert        = { type = "bool",   value = false, hint = "Flip if wheels turn the wrong way" },
        _angle        = { type = "float",  value = 0.0,  hint = "Internal: current smoothed angle (deg)" },
    }
end

function on_update()
    -- Read the left stick X (-1 = left, +1 = right) with a deadzone so
    -- a controller at rest doesn't drift the wheels off-center.
    local x = gamepad_left_x
    if math.abs(x) < deadzone then
        x = 0.0
    end

    local sign = 1.0
    if invert then sign = -1.0 end

    local target = x * max_angle_deg * sign

    -- Frame-rate independent exponential smoothing toward target so the
    -- wheels ease in/out instead of snapping each frame.
    local t = 1.0 - math.exp(-smoothing * delta)
    _angle = _angle + (target - _angle) * t

    set_child_rotation(front_left,  0.0, _angle, 0.0)
    set_child_rotation(front_right, 0.0, _angle, 0.0)
end
