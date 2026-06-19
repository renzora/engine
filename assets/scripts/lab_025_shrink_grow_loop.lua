-- lab_025_shrink_grow_loop: slow uniform shrink/grow that loops forever.
-- A cosine starts the cycle at full size (cos(0)=1) so the entity begins at its
-- authored scale and eases down first, avoiding a jarring pop on spawn.

function props()
    return {
        min_scale = { value = 0.5, hint = "Smallest size", tab = "Loop" },
        max_scale = { value = 1.5, hint = "Largest size", tab = "Loop" },
        frequency = { value = 0.25, hint = "Cycles per second", tab = "Loop" },
    }
end

function on_update()
    local t = math.cos(elapsed * frequency * math.pi * 2.0) * 0.5 + 0.5
    set_scale_uniform(lerp(min_scale, max_scale, t))
end
