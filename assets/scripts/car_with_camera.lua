-- Car + Follow Camera
--
-- Arcade car controller that also drives a chase camera.
-- Attach this to the CAR entity; set `camera` to the camera entity's name.
-- W/S accelerate/reverse, A/D steer, Space handbrake.

function props()
    return {
        -- Car
        max_speed     = { type = "float",  value = 20.0, hint = "Top forward speed" },
        max_reverse   = { type = "float",  value = 8.0,  hint = "Top reverse speed" },
        acceleration  = { type = "float",  value = 12.0, hint = "Acceleration (units/s^2)" },
        brake_force   = { type = "float",  value = 20.0, hint = "Brake deceleration" },
        friction      = { type = "float",  value = 3.0,  hint = "Passive deceleration" },
        turn_speed    = { type = "float",  value = 90.0, hint = "Max turn rate (deg/s)" },
        handbrake     = { type = "float",  value = 35.0, hint = "Handbrake deceleration" },

        -- Camera
        camera         = { type = "string", value = "Camera", hint = "Name of camera entity to drive" },
        cam_distance   = { type = "float",  value = 8.0,      hint = "Trail distance behind car" },
        cam_height     = { type = "float",  value = 3.5,      hint = "Height above car" },
        look_height    = { type = "float",  value = 1.0,      hint = "Aim point height above car origin" },
        follow_smooth  = { type = "float",  value = 6.0,      hint = "Position smoothing (higher = snappier)" },
        invert_forward = { type = "bool",   value = true,     hint = "Flip if camera is on the wrong side" },
        yaw_offset     = { type = "float",  value = 0.0,      hint = "Extra yaw (deg) applied to heading" },

        -- Internal
        _velocity = { type = "float", value = 0.0, hint = "Internal: signed speed" },
        _yaw      = { type = "float", value = 0.0, hint = "Internal: heading (deg)" },
        _cam_x    = { type = "float", value = 0.0, hint = "Internal: smoothed cam x" },
        _cam_y    = { type = "float", value = 0.0, hint = "Internal: smoothed cam y" },
        _cam_z    = { type = "float", value = 0.0, hint = "Internal: smoothed cam z" },
        _cam_init = { type = "bool",  value = false, hint = "Internal: cam state initialized" },
    }
end

function on_ready()
    _yaw = rotation_y
    _velocity = 0.0
    _cam_init = false
    lock_cursor()
end

function on_update()
    ----------------------------------------------------------------
    -- CAR MOVEMENT
    ----------------------------------------------------------------
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
        if _velocity > 0 then
            _velocity = math.max(0, _velocity - friction * delta)
        elseif _velocity < 0 then
            _velocity = math.min(0, _velocity + friction * delta)
        end
    end

    if is_key_pressed("Space") then
        if _velocity > 0 then
            _velocity = math.max(0, _velocity - handbrake * delta)
        elseif _velocity < 0 then
            _velocity = math.min(0, _velocity + handbrake * delta)
        end
    end

    if _velocity > max_speed then _velocity = max_speed end
    if _velocity < -max_reverse then _velocity = -max_reverse end

    steer = 0
    if is_key_pressed("KeyA") or is_key_pressed("ArrowLeft")  then steer = steer + 1 end
    if is_key_pressed("KeyD") or is_key_pressed("ArrowRight") then steer = steer - 1 end

    speed_ratio = math.abs(_velocity) / max_speed
    if speed_ratio > 1 then speed_ratio = 1 end
    direction = 1
    if _velocity < 0 then direction = -1 end

    _yaw = _yaw + steer * turn_speed * speed_ratio * direction * delta
    set_rotation(0.0, _yaw, 0.0)

    -- Mesh faces -Z, so negate forward
    rad = math.rad(_yaw)
    fwd_x = -math.sin(rad)
    fwd_z = -math.cos(rad)
    translate(fwd_x * _velocity * delta, 0, fwd_z * _velocity * delta)

    ----------------------------------------------------------------
    -- FOLLOW CAMERA
    ----------------------------------------------------------------
    -- Car position (just computed) — read back so we chase the post-translate value
    tx = get("Transform.translation.x")
    ty = get("Transform.translation.y")
    tz = get("Transform.translation.z")
    if tx == nil or ty == nil or tz == nil then return end

    -- Heading-based forward for camera placement (invert_forward flips sign)
    cam_yaw = math.rad(_yaw) + math.rad(yaw_offset)
    sign = 1.0
    if invert_forward then sign = -1.0 end
    cfwd_x = math.sin(cam_yaw) * sign
    cfwd_z = math.cos(cam_yaw) * sign

    desired_x = tx - cfwd_x * cam_distance
    desired_y = ty + cam_height
    desired_z = tz - cfwd_z * cam_distance

    -- Initialize smoothed cam state on first frame so it doesn't snap from origin
    if not _cam_init then
        _cam_x = desired_x
        _cam_y = desired_y
        _cam_z = desired_z
        _cam_init = true
    end

    t = 1.0 - math.exp(-follow_smooth * delta)
    _cam_x = _cam_x + (desired_x - _cam_x) * t
    _cam_y = _cam_y + (desired_y - _cam_y) * t
    _cam_z = _cam_z + (desired_z - _cam_z) * t

    set_on(camera, "Transform.translation.x", _cam_x)
    set_on(camera, "Transform.translation.y", _cam_y)
    set_on(camera, "Transform.translation.z", _cam_z)

    -- Aim the camera at (tx, ty + look_height, tz).
    -- Bevy cameras face -Z by default, so yaw = atan2(-dx, -dz), pitch = atan2(dy, |horiz|).
    -- Build quaternion from YXZ Euler (yaw around Y, then pitch around X).
    dx = tx - _cam_x
    dy = (ty + look_height) - _cam_y
    dz = tz - _cam_z
    horiz = math.sqrt(dx * dx + dz * dz)
    cam_look_yaw   = math.atan2(-dx, -dz)
    cam_look_pitch = math.atan2(dy, horiz)

    hy = cam_look_yaw   * 0.5
    hp = cam_look_pitch * 0.5
    sy = math.sin(hy); cy = math.cos(hy)
    sp = math.sin(hp); cp = math.cos(hp)

    qx = cy * sp
    qy = sy * cp
    qz = -sy * sp
    qw = cy * cp

    set_on(camera, "Transform.rotation.x", qx)
    set_on(camera, "Transform.rotation.y", qy)
    set_on(camera, "Transform.rotation.z", qz)
    set_on(camera, "Transform.rotation.w", qw)
end
