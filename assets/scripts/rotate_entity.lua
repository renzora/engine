-- rotate_entity: spins the entity around any combination of axes.
-- Speeds are degrees-per-second and get multiplied by `delta`, so the rotation
-- rate stays the same whether the game is running at 30 or 240 FPS. Leave an
-- axis at 0 to keep it still — setting all three gives a tumbling motion.

function props()
    return {
        speed_x = { value = 0.0,  hint = "Degrees per second around X (pitch)", tab = "Rotation" },
        speed_y = { value = 90.0, hint = "Degrees per second around Y (yaw)",   tab = "Rotation" },
        speed_z = { value = 0.0,  hint = "Degrees per second around Z (roll)",  tab = "Rotation" },
        enabled = { value = true, hint = "Uncheck to freeze the rotation",      tab = "Rotation" },
    }
end

function on_update()
    if not enabled then
        return
    end

    rotate(speed_x * delta, speed_y * delta, speed_z * delta)
end
