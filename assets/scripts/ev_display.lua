-- Camera EV Display
--
-- Attach to a UI text widget. Updates its own text each frame with the
-- live camera EV-100 value reported by `renzora_auto_exposure`'s GPU
-- luminance readback.
--
-- The `camera_ev` global is populated every frame from
-- `renzora::core::CameraExposureState`. It reads ~0.0 when auto-exposure
-- isn't running yet (first frame, AE disabled, no readback complete).

function props()
    return {
        update_interval = { type = "float",  value = 0.1,   hint = "Seconds between text refreshes (smooths jitter)" },
        label           = { type = "string", value = "EV: ", hint = "Prefix shown before the number" },
        _accum_time     = { type = "float",  value = 0.0,   hint = "Internal: time since last refresh" },
    }
end

function on_ready()
    _accum_time = 0.0
    set("text.content", label .. "--")
end

function on_update()
    _accum_time = _accum_time + delta
    if _accum_time >= update_interval then
        set("text.content", label .. string.format("%.2f", camera_ev))
        _accum_time = 0.0
    end
end
