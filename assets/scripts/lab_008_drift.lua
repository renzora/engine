-- lab_008_drift: slow constant drift along a configurable direction.
-- Uses translate() with delta so the entity glides at a steady world-units-per-
-- second pace regardless of frame rate; nothing pulls it back, so it wanders off.

function props()
    return {
        dir_x = { value = 0.1, hint = "Drift X per second", tab = "Drift" },
        dir_y = { value = 0.0, hint = "Drift Y per second", tab = "Drift" },
        dir_z = { value = 0.05, hint = "Drift Z per second", tab = "Drift" },
    }
end

function on_update()
    translate(dir_x * delta, dir_y * delta, dir_z * delta)
end
