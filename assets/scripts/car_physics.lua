-- Car Physics Movement
--
-- Physics-driven car controller. Attach to an entity with a Dynamic RigidBody
-- (PhysicsBodyData) and a Box/Capsule collider.
--
-- Recommended body setup:
--   body_type = RigidBody, mass = 1.0
--   linear_damping = 0.3, angular_damping = 1.0
--   lock_rotation_x = true, lock_rotation_z = true (prevents flipping)
--
-- Controls: W/S throttle, A/D steer, Space handbrake.

function props()
    return {
        engine_force  = { type = "float", value = 25.0, hint = "Forward thrust" },
        brake_force   = { type = "float", value = 40.0, hint = "Reverse thrust when braking" },
        turn_speed    = { type = "float", value = 120.0, hint = "Steering rate (deg/s)" },
        handbrake_damp= { type = "float", value = 6.0,  hint = "Velocity damping when handbrake held" },
        min_turn_speed= { type = "float", value = 0.3,  hint = "Yaw still responds below this (0 = must be moving)" },
        _yaw          = { type = "float", value = 0.0,  hint = "Internal: heading" },
        _throttling   = { type = "bool",  value = false, hint = "Internal: was throttle pressed" },
    }
end

function on_ready()
    _yaw = rotation_y
end

function on_update()
    -- Steering input
    steer = 0
    if is_key_pressed("KeyA") or is_key_pressed("ArrowLeft")  then steer = steer + 1 end
    if is_key_pressed("KeyD") or is_key_pressed("ArrowRight") then steer = steer - 1 end

    _yaw = _yaw + steer * turn_speed * delta
    set_rotation(0.0, _yaw, 0.0)

    -- Forward vector from yaw
    rad = math.rad(_yaw)
    fwd_x = math.sin(rad)
    fwd_z = math.cos(rad)

    -- Throttle / brake
    throttle = 0
    if is_key_pressed("KeyW") or is_key_pressed("ArrowUp")   then throttle = throttle + 1 end
    if is_key_pressed("KeyS") or is_key_pressed("ArrowDown") then throttle = throttle - 1 end

    if throttle > 0 then
        apply_force(fwd_x * engine_force, 0.0, fwd_z * engine_force)
    elseif throttle < 0 then
        apply_force(-fwd_x * brake_force, 0.0, -fwd_z * brake_force)
    end

    -- Handbrake: temporarily bleed horizontal velocity.
    -- Approximated by writing zero horizontal velocity while keeping gravity pull via damping.
    if is_key_pressed("Space") then
        set_on("self", "PhysicsBodyData.linear_damping", handbrake_damp)
    else
        set_on("self", "PhysicsBodyData.linear_damping", 0.3)
    end
end
