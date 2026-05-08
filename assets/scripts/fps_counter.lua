-- FPS Counter
--
-- Attach to a UI text widget. Updates its own Text component each frame
-- with the current frame rate, averaged over a short window so the number
-- doesn't strobe.
--
-- Uses generic reflection (`set`) so it works on the script's own entity
-- with no name/target lookup. The same pattern works for any reflected
-- component: `set("Node.width", v)`, `set("UiBorderRadius.top_left", v)`,
-- `set("UiTextStyle.size", v)`, `set("BackgroundColor.color", { r=…, g=…, b=…, a=… })`, etc.

function props()
    return {
        update_interval = { type = "float",  value = 0.25,    hint = "Seconds between text refreshes" },
        label           = { type = "string", value = "FPS: ", hint = "Prefix shown before the number" },
        _accum_time     = { type = "float",  value = 0.0,     hint = "Internal: time since last refresh" },
        _accum_frames   = { type = "float",  value = 0.0,     hint = "Internal: frames since last refresh" },
    }
end

function on_ready()
    _accum_time = 0.0
    _accum_frames = 0.0
    set("text.content", label .. "--")
end

function on_update()
    _accum_time = _accum_time + delta
    _accum_frames = _accum_frames + 1.0

    if _accum_time >= update_interval then
        local fps = _accum_frames / _accum_time
        set("text.content", label .. string.format("%.0f", fps))
        _accum_time = 0.0
        _accum_frames = 0.0
    end
end
