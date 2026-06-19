-- lab_018_flicker: random on/off flicker, like a failing light or fire.
-- Visibility is rerolled each frame against a probability, so the pattern is
-- irregular rather than the even cadence of a timed blink.

function props()
    return {
        on_chance = { value = 0.8, hint = "Probability visible per frame", tab = "Flicker" },
    }
end

function on_update()
    set_visibility(math.random() < on_chance)
end
