-- lab_022_figure_eight: traces a lemniscate (figure-eight) in the XZ plane.
-- X uses sin(t) and Z uses sin(2t); the 2:1 frequency ratio is what crosses the
-- path through the center to make the eight. Positions are absolute from elapsed.

function props()
    return {
        size = { value = 2.0, hint = "Path extent", tab = "Path" },
        speed = { value = 0.4, hint = "Loops per second", tab = "Path" },
        height = { value = 1.0, hint = "Y height", tab = "Path" },
    }
end

function on_update()
    local a = elapsed * speed * math.pi * 2.0
    set_position(math.sin(a) * size, height, math.sin(a * 2.0) * size * 0.5)
end
