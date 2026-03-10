-- Ball controller with gamepad + keyboard
-- Attach to the ball entity (needs a dynamic rigid body + collider)

function props()
    return {
        speed = { value = 500.0, hint = "Roll force strength" },
        jump_force = { value = 8.0, hint = "Jump impulse strength" },
        respawn_y = { value = -10.0, hint = "Y position that triggers respawn" },
        deadzone = { value = 0.15, hint = "Stick deadzone" },
        camera_name = { value = "Camera 3D", hint = "Name of camera entity" },
        spawn_x = { value = 0.0, hint = "Internal" },
        spawn_y = { value = 0.0, hint = "Internal" },
        spawn_z = { value = 0.0, hint = "Internal" },
        can_jump = { value = 0.0, hint = "Internal" },
        initialized = { value = 0.0, hint = "Internal" },
    }
end

function on_update()
    local _speed = speed or 500.0
    local _jump_force = jump_force or 8.0
    local _respawn_y = respawn_y or -10.0
    local _deadzone = deadzone or 0.15
    local _camera_name = camera_name or "Camera 3D"

    -- store spawn point on first frame
    if not initialized or initialized < 0.5 then
        spawn_x = position_x
        spawn_y = position_y
        spawn_z = position_z
        initialized = 1.0
        print_log("[Ball] ready at " .. spawn_x .. ", " .. spawn_y .. ", " .. spawn_z)
    end

    -- read left stick
    local move_x = gamepad_left_x or 0
    local move_z = -(gamepad_left_y or 0)
    if math.abs(move_x) < _deadzone then move_x = 0 end
    if math.abs(move_z) < _deadzone then move_z = 0 end

    -- add keyboard input
    if is_key_pressed("KeyW") then move_z = move_z + 1 end
    if is_key_pressed("KeyS") then move_z = move_z - 1 end
    if is_key_pressed("KeyA") then move_x = move_x - 1 end
    if is_key_pressed("KeyD") then move_x = move_x + 1 end

    -- normalize
    local len = math.sqrt(move_x * move_x + move_z * move_z)
    if len > 1.0 then
        move_x = move_x / len
        move_z = move_z / len
    end

    -- camera-relative movement
    if move_x ~= 0 or move_z ~= 0 then
        local cam_pos = get_on(_camera_name, "Transform.translation")
        if cam_pos then
            local dx = position_x - cam_pos.x
            local dz = position_z - cam_pos.z
            local d = math.sqrt(dx * dx + dz * dz)
            if d > 0.001 then
                local fwd_x = dx / d
                local fwd_z = dz / d
                local right_x = -fwd_z
                local right_z = fwd_x
                local rx = move_x * right_x + move_z * fwd_x
                local rz = move_x * right_z + move_z * fwd_z
                move_x = rx
                move_z = rz
            end
        end

        print_log("[Ball] input x=" .. string.format("%.2f", move_x) .. " z=" .. string.format("%.2f", move_z))
        apply_force(move_x * _speed, 0, move_z * _speed)
    end

    -- jump (X on PS5 / Space on keyboard)
    local _can_jump = can_jump or 0
    local jump_pressed = gamepad_south or is_key_just_pressed("Space")
    if jump_pressed and _can_jump > 0.5 then
        apply_impulse(0, _jump_force, 0)
        can_jump = 0.0
        print_log("[Ball] jump!")
    end

    -- reset jump when grounded
    if is_colliding then
        can_jump = 1.0
    end

    -- respawn if fallen
    if position_y < _respawn_y then
        set_position(spawn_x or 0, spawn_y or 1, spawn_z or 0)
        set_velocity(0, 0, 0)
        print_log("[Ball] respawned")
    end
end
