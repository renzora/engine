-- lab_007_orbit: orbits the world origin in the XZ plane at a fixed radius.
-- Position is recomputed from elapsed each frame (absolute, not incremental)
-- so floating-point drift can never let the orbit spiral in or out.

function props()
    return {
        radius = { value = 3.0, hint = "Orbit radius", tab = "Orbit" },
        speed = { value = 0.5, hint = "Revolutions per second", tab = "Orbit" },
        height = { value = 1.0, hint = "Orbit Y height", tab = "Orbit" },
    }
end

function on_update()
    local a = elapsed * speed * math.pi * 2.0
    set_position(math.cos(a) * radius, height, math.sin(a) * radius)
end
