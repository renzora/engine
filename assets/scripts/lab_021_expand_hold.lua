-- lab_021_expand_hold: grows from zero to full size once, then holds.
-- A one-shot eased entrance (no looping): t marches to 1 and stays, so this is
-- a spawn/"pop-in" animation rather than a repeating pulse.

local t = 0.0

function props()
    return {
        target_scale = { value = 1.0, hint = "Final uniform scale", tab = "Expand" },
        grow_speed = { value = 2.0, hint = "Expansion rate", tab = "Expand" },
    }
end

function on_ready()
    set_scale_uniform(0.0)
end

function on_update()
    if t >= 1.0 then return end
    t = clamp(t + grow_speed * delta, 0.0, 1.0)
    set_scale_uniform(target_scale * t)
end
