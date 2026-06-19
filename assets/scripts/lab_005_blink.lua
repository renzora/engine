-- lab_005_blink: toggles visibility on a fixed interval.
-- We derive the on/off state from elapsed via a modulo instead of tracking a
-- timer, so the blink stays in lockstep with wall-clock time even after a hitch.

function props()
    return {
        period = { value = 0.5, hint = "Seconds per on/off half", tab = "Blink" },
    }
end

function on_update()
    local phase = (elapsed % (period * 2.0)) < period
    set_visibility(phase)
end
