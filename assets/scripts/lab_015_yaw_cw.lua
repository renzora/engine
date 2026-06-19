-- lab_015_yaw_cw: steady clockwise yaw using absolute angle from elapsed.
-- Setting rotation_y directly from elapsed (rather than accumulating with
-- rotate) keeps the angle exact and the negative sign makes it clockwise.

function props()
    return {
        speed = { value = 60.0, hint = "Degrees per second (clockwise)", tab = "Yaw" },
    }
end

function on_update()
    set_rotation(rotation_x, -(elapsed * speed) % 360.0, rotation_z)
end
