-- Car Movement
--
-- Arcade-style car controller with momentum and steering.
-- W/S accelerate/reverse, A/D steer, Space handbrake.
-- Steering only affects direction while moving.

function props()
    return {
        max_speed     = { type = "float", value = 20.0, hint = "Top forward speed" },
        max_reverse   = { type = "float", value = 8.0,  hint = "Top reverse speed" },
        acceleration  = { type = "float", value = 12.0, hint = "Acceleration (units/s^2)" },
        brake_force   = { type = "float", value = 20.0, hint = "Brake deceleration" },
        friction      = { type = "float", value = 3.0,  hint = "Passive deceleration" },
        turn_speed    = { type = "float", value = 90.0, hint = "Max turn rate (deg/s)" },
        handbrake     = { type = "float", value = 35.0, hint = "Handbrake deceleration" },
        _velocity     = { type = "float", value = 0.0,  hint = "Internal: signed speed" },
        _yaw          = { type = "float", value = 0.0,  hint = "Internal: heading" },
    }
end

function on_ready()
    _yaw = rotation_y
    _velocity = 0.0
end

function on_update()
    -- Throttle / brake input
    throttle = 0
    if is_key_pressed("KeyW") or is_key_pressed("ArrowUp")   then throttle = throttle + 1 end
    if is_key_pressed("KeyS") or is_key_pressed("ArrowDown") then throttle = throttle - 1 end

    if throttle > 0 then
        _velocity = _velocity + acceleration * delta
    elseif throttle < 0 then
        if _velocity > 0 then
            _velocity = _velocity - brake_force * delta
        else
            _velocity = _velocity - acceleration * delta
        end
    else
        -- Coast: bleed speed toward zero
        if _velocity > 0 then
            _velocity = math.max(0, _velocity - friction * delta)
        elseif _velocity < 0 then
            _velocity = math.min(0, _velocity + friction * delta)
        end
    end

    -- Handbrake
    if is_key_pressed("Space") then
        if _velocity > 0 then
            _velocity = math.max(0, _velocity - handbrake * delta)
        elseif _velocity < 0 then
            _velocity = math.min(0, _velocity + handbrake * delta)
        end
    end

    -- Clamp to limits
    if _velocity > max_speed then _velocity = max_speed end
    if _velocity < -max_reverse then _velocity = -max_reverse end

    -- Steering: scales with speed, reverses when going backwards
    steer = 0
    if is_key_pressed("KeyA") or is_key_pressed("ArrowLeft")  then steer = steer + 1 end
    if is_key_pressed("KeyD") or is_key_pressed("ArrowRight") then steer = steer - 1 end

    speed_ratio = math.abs(_velocity) / max_speed
    if speed_ratio > 1 then speed_ratio = 1 end

    direction = 1
    if _velocity < 0 then direction = -1 end

    _yaw = _yaw + steer * turn_speed * speed_ratio * direction * delta
    set_rotation(0.0, _yaw, 0.0)

    -- Apply motion along heading
    rad = math.rad(_yaw)
    fwd_x = math.sin(rad)
    fwd_z = math.cos(rad)

    translate(fwd_x * _velocity * delta, 0, fwd_z * _velocity * delta)
end
