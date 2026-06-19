-- lab_001_spin: rotates the entity continuously around its Y axis.
-- Uses rotate() with a per-frame delta so the spin speed is frame-rate
-- independent (degrees-per-second * delta), not tied to raw frame count.

function props()
    return {
        speed = { value = 90.0, hint = "Degrees per second around Y", tab = "Spin" },
    }
end

function on_update()
    rotate(0.0, speed * delta, 0.0)
end
