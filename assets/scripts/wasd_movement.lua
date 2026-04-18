-- WASD Movement
--
-- Camera-relative movement. Attach to the same entity as fps_controller.lua.
-- Uses rotation_y (yaw) from the entity's own transform.

function props()
    return {
        move_speed        = { type = "float", value = 10.0, hint = "Movement speed" },
        sprint_multiplier = { type = "float", value = 2.0,  hint = "Sprint speed multiplier" },
    }
end

function on_update()
    speed = move_speed
    if is_key_pressed("ShiftLeft") then
        speed = speed * sprint_multiplier
    end

    -- Direction from entity's own yaw
    rad = math.rad(rotation_y)
    fwd_x = -math.sin(rad)
    fwd_z = -math.cos(rad)
    right_x =  math.cos(rad)
    right_z = -math.sin(rad)

    mx = 0
    mz = 0

    if is_key_pressed("KeyW") or is_key_pressed("ArrowUp")    then mx = mx + fwd_x;   mz = mz + fwd_z end
    if is_key_pressed("KeyS") or is_key_pressed("ArrowDown")  then mx = mx - fwd_x;   mz = mz - fwd_z end
    if is_key_pressed("KeyD") or is_key_pressed("ArrowRight") then mx = mx + right_x;  mz = mz + right_z end
    if is_key_pressed("KeyA") or is_key_pressed("ArrowLeft")  then mx = mx - right_x;  mz = mz - right_z end

    -- Normalize diagonal
    len = math.sqrt(mx * mx + mz * mz)
    if len > 0 then
        mx = mx / len
        mz = mz / len
    end

    -- Vertical
    my = 0
    if is_key_pressed("Space")       then my = my + 1 end
    if is_key_pressed("ControlLeft") then my = my - 1 end

    translate(mx * speed * delta, my * speed * delta, mz * speed * delta)
end
