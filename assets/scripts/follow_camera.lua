-- Follow Camera
--
-- Chase camera that trails a target entity (e.g. a car) from behind and above.
-- Reads the target's world position and rotation, then smoothly interpolates.
-- Attach to a camera entity; set `target` to the target entity's name.

function props()
    return {
        target        = { type = "string", value = "Car",  hint = "Name of entity to follow" },
        distance      = { type = "float",  value = 8.0,    hint = "Trail distance behind target" },
        height        = { type = "float",  value = 3.5,    hint = "Height above target" },
        look_height   = { type = "float",  value = 1.0,    hint = "Aim point height above target origin" },
        follow_smooth = { type = "float",  value = 6.0,    hint = "Position smoothing (higher = snappier)" },
    }
end

function on_ready()
    lock_cursor()
end

function on_update()
    -- Target position
    tx = get_on(target, "Transform.translation.x")
    ty = get_on(target, "Transform.translation.y")
    tz = get_on(target, "Transform.translation.z")
    if tx == nil or ty == nil or tz == nil then return end

    -- Target rotation (quaternion) -> yaw in YXZ order
    qx = get_on(target, "Transform.rotation.x") or 0.0
    qy = get_on(target, "Transform.rotation.y") or 0.0
    qz = get_on(target, "Transform.rotation.z") or 0.0
    qw = get_on(target, "Transform.rotation.w") or 1.0
    yaw = math.atan2(2.0 * (qw * qy + qx * qz), 1.0 - 2.0 * (qy * qy + qx * qx))

    -- Desired camera position: behind the target along its forward
    fwd_x = math.sin(yaw)
    fwd_z = math.cos(yaw)
    desired_x = tx + fwd_x * distance
    desired_y = ty + height
    desired_z = tz + fwd_z * distance

    -- Exponential smoothing toward desired position
    t = 1.0 - math.exp(-follow_smooth * delta)
    nx = position_x + (desired_x - position_x) * t
    ny = position_y + (desired_y - position_y) * t
    nz = position_z + (desired_z - position_z) * t

    set_position(nx, ny, nz)
    look_at(tx, ty + look_height, tz)
end
