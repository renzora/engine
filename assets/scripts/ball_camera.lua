-- Third-person camera for ball controller
-- Attach to the Camera 3D entity

function props()
    return {
        target_name = { value = "Sphere", hint = "Name of ball entity to follow" },
        distance = { value = 12.0, hint = "Distance from ball" },
        height = { value = 8.0, hint = "Height above ball" },
        look_offset_y = { value = 0.5, hint = "Y offset for look-at point" },
        smoothing = { value = 5.0, hint = "Follow smoothing (higher = snappier)" },
        stick_sensitivity = { value = 120.0, hint = "Right stick rotation speed (deg/sec)" },
        deadzone = { value = 0.15, hint = "Stick deadzone" },
        yaw = { value = 0.0, hint = "Internal" },
        pitch = { value = 20.0, hint = "Camera pitch angle" },
        pitch_min = { value = 5.0, hint = "Min pitch" },
        pitch_max = { value = 70.0, hint = "Max pitch" },
        cam_x = { value = 0.0, hint = "Internal" },
        cam_y = { value = 0.0, hint = "Internal" },
        cam_z = { value = 0.0, hint = "Internal" },
        initialized = { value = 0.0, hint = "Internal" },
    }
end

function on_update()
    local pos = get_on(target_name, "Transform.translation")
    if not pos then return end

    -- right stick rotation
    local stick_x = gamepad_right_x or 0
    local stick_y = gamepad_right_y or 0
    if math.abs(stick_x) < deadzone then stick_x = 0 end
    if math.abs(stick_y) < deadzone then stick_y = 0 end

    yaw = yaw - stick_x * stick_sensitivity * delta
    pitch = pitch - stick_y * stick_sensitivity * delta
    pitch = clamp(pitch, pitch_min, pitch_max)

    -- calculate camera position on a sphere around the ball
    local yaw_rad = math.rad(yaw)
    local pitch_rad = math.rad(pitch)

    local horizontal = math.cos(pitch_rad) * distance
    local goal_x = pos.x - math.sin(yaw_rad) * horizontal
    local goal_z = pos.z - math.cos(yaw_rad) * horizontal
    local goal_y = pos.y + height + math.sin(pitch_rad) * distance

    if initialized < 0.5 then
        cam_x = goal_x
        cam_y = goal_y
        cam_z = goal_z
        initialized = 1.0
        print_log("[Camera] following " .. target_name)
    end

    local t = clamp(smoothing * delta, 0, 1)
    cam_x = lerp(cam_x, goal_x, t)
    cam_y = lerp(cam_y, goal_y, t)
    cam_z = lerp(cam_z, goal_z, t)

    set_position(cam_x, cam_y, cam_z)
    look_at(pos.x, pos.y + look_offset_y, pos.z)
end
