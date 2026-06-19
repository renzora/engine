-- lab_009_breathing: slow eased "breathing" scale, like a resting chest.
-- A low frequency plus a squared sine gives a longer pause at the small end,
-- reading as an inhale/exhale rhythm rather than a mechanical pulse.

function props()
    return {
        min_scale = { value = 0.9, hint = "Exhaled size", tab = "Breathe" },
        max_scale = { value = 1.1, hint = "Inhaled size", tab = "Breathe" },
        frequency = { value = 0.3, hint = "Breaths per second", tab = "Breathe" },
    }
end

function on_update()
    local s = math.sin(elapsed * frequency * math.pi * 2.0)
    local t = s * s
    set_scale_uniform(lerp(min_scale, max_scale, t))
end
