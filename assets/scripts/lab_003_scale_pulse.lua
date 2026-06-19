-- lab_003_scale_pulse: pulses uniform scale with a sine wave.
-- Centered on 1.0 so the entity grows and shrinks symmetrically around its
-- authored size rather than only ever growing.

function props()
    return {
        amount = { value = 0.2, hint = "Scale variation", tab = "Pulse" },
        frequency = { value = 1.5, hint = "Cycles per second", tab = "Pulse" },
    }
end

function on_update()
    local s = 1.0 + math.sin(elapsed * frequency * math.pi * 2.0) * amount
    set_scale_uniform(s)
end
