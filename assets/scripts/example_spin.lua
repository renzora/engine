-- Spins the entity continuously.
-- Demonstrates: props(), on_update, the `delta` global, rotate().
-- Attach to any 3D entity; tweak speed/axis in the inspector.

function props()
    return {
        speed = { type = "float",  value = 45.0, hint = "degrees per second" },
        axis  = { type = "string", value = "y",  hint = "x | y | z" },
    }
end

function on_update()
    local d = speed * delta            -- frame-rate independent
    if axis == "x" then
        rotate(d, 0, 0)
    elseif axis == "z" then
        rotate(0, 0, d)
    else
        rotate(0, d, 0)
    end
end
