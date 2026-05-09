-- Sphere Roll
--
-- Camera-relative arrow-key rolling with momentum. Velocity exponentially
-- approaches the input direction while a key is held and decays back to rest
-- when released, so direction changes arc naturally instead of snapping.
-- Attach to the sphere; point `camera` at the camera entity to drive
-- "forward" / "right" relative to where it's looking.

function props()
    return {
        camera        = { type = "string", value = "Camera", hint = "Name of camera entity (defines forward direction)" },
        radius        = { type = "float",  value = 0.5,      hint = "Sphere radius (units). Match mesh size for no-slip rolling. Engine's primitive sphere is 0.5; multiply by your scale_y if scaled." },
        max_speed     = { type = "float",  value = 6.0,      hint = "Top linear speed (units/sec)" },
        accel_rate    = { type = "float",  value = 8.0,      hint = "How quickly velocity catches up to input (1/sec; higher = snappier)" },
        friction_rate = { type = "float",  value = 2.5,      hint = "How quickly velocity decays with no input (1/sec; lower = rolls farther)" },
        _vx           = { type = "float",  value = 0.0,      hint = "Internal: x velocity" },
        _vz           = { type = "float",  value = 0.0,      hint = "Internal: z velocity" },
    }
end

function on_update()
    -- Raw arrow-key input on the XZ plane
    local ix = 0
    local iz = 0
    if is_key_pressed("ArrowUp")    then iz = iz + 1 end
    if is_key_pressed("ArrowDown")  then iz = iz - 1 end
    if is_key_pressed("ArrowLeft")  then ix = ix - 1 end
    if is_key_pressed("ArrowRight") then ix = ix + 1 end

    local len = math.sqrt(ix * ix + iz * iz)
    if len > 0 then
        ix = ix / len
        iz = iz / len
    end

    -- Yaw extracted from the camera's quaternion (YXZ order, same formula as follow_camera.lua)
    local yaw = 0.0
    if camera ~= nil and camera ~= "" then
        local qx = get_on(camera, "Transform.rotation.x") or 0.0
        local qy = get_on(camera, "Transform.rotation.y") or 0.0
        local qz = get_on(camera, "Transform.rotation.z") or 0.0
        local qw = get_on(camera, "Transform.rotation.w") or 1.0
        yaw = math.atan2(2.0 * (qw * qy + qx * qz), 1.0 - 2.0 * (qy * qy + qx * qx))
    end

    -- Camera-local forward (-Z) and right (+X) projected onto the ground plane
    local fwd_x   = -math.sin(yaw)
    local fwd_z   = -math.cos(yaw)
    local right_x =  math.cos(yaw)
    local right_z = -math.sin(yaw)

    -- World-space input direction: Up = camera-forward, Right = camera-right
    local dir_x = ix * right_x + iz * fwd_x
    local dir_z = ix * right_z + iz * fwd_z

    -- Exponential approach toward target velocity. With input we accelerate
    -- toward (dir * max_speed); without input we decay toward zero. Using
    -- different rates lets the ball respond quickly but coast on release.
    local target_vx = dir_x * max_speed
    local target_vz = dir_z * max_speed
    local rate = (len > 0) and accel_rate or friction_rate
    local k = 1.0 - math.exp(-rate * delta)
    _vx = _vx + (target_vx - _vx) * k
    _vz = _vz + (target_vz - _vz) * k

    local dx = _vx * delta
    local dz = _vz * delta
    if dx == 0.0 and dz == 0.0 then return end

    translate(dx, 0.0, dz)

    -- omega = v / R (no-slip). +Z motion pitches +X; +X motion rolls -Z.
    if radius > 0.0 then
        local s = (180.0 / math.pi) / radius
        rotate(dz * s, 0.0, -dx * s)
    end
end
