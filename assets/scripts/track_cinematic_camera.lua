-- Track Cinematic Camera
--
-- Showcases a racing track (or any entity) from a series of fixed overhead
-- vantage points. Each "shot" is an elevated position at a different angle,
-- distance and height around the target. The camera slowly pans during a shot,
-- holds for `hold_time`, then transitions to the next shot and repeats.
--
-- Attach this to the CAMERA entity.
-- Set `target` to the NAME or TAG of the entity to frame (e.g. the track or a car).
-- Press N to manually jump to the next shot.
--
-- TRANSITIONS (crossfade / wipe / slide / iris):
--   In the inspector, add the "Screen Transition" effect to THIS camera entity.
--   This script then animates it at each cut. With no effect added (or
--   transition = "cut"), the camera just hard-cuts.

-- Camera positions, relative to the target. Edit / add freely.
--   angle  = compass angle around the target, in degrees (0 = +Z)
--   dist   = horizontal distance from the target
--   height = height above the target
--   pan    = slow orbit speed during the shot, deg/sec (0 = static, +/- = direction)
local SHOTS = {
    { angle =   0, dist = 30, height = 24, pan =  6 },   -- high wide establishing shot
    { angle = 100, dist = 18, height = 13, pan = -8 },   -- lower banked sweep
    { angle = 200, dist = 40, height = 34, pan =  4 },   -- top-down birds eye
    { angle = 300, dist = 14, height =  9, pan = 12 },   -- close fast orbit
}

-- transition name -> ScreenTransitionSettings.mode (see renzora_screen_transition)
local MODES = { crossfade = 0, wipe = 1, slide = 2, iris = 3 }

function props()
    return {
        target           = { type = "string", value = "Track",     hint = "Name or tag of entity to frame" },
        hold_time        = { type = "float",  value = 5.0,         hint = "Seconds to hold each position before cutting" },
        look_height      = { type = "float",  value = 0.0,         hint = "Aim point height above target origin" },
        auto_cycle       = { type = "bool",   value = true,        hint = "Auto-advance; off = only advance on N key" },
        transition       = { type = "string", value = "crossfade", hint = "cut | crossfade | wipe | slide | iris" },
        transition_time  = { type = "float",  value = 0.6,         hint = "Transition duration in seconds" },
        transition_dir   = { type = "float",  value = 0.0,         hint = "Wipe/slide direction: 0=L 1=R 2=Up 3=Down" },
        transition_soft  = { type = "float",  value = 0.04,        hint = "Edge softness for wipe/iris" },
    }
end

function on_ready()
    _shot         = 1
    _timer        = 0.0      -- time spent in current shot
    _orbit        = 0.0      -- accumulated pan within current shot (deg)
    _transitioning = false
    _t_clock      = 0.0      -- elapsed time within the current transition
    -- Idle the effect (1.0 = show the live frame; the snapshot keeps refreshing).
    set("ScreenTransitionSettings.progress", 1.0)
end

-- Freeze the outgoing frame and start a transition to the just-selected shot.
function start_transition()
    local mode = MODES[transition]
    if mode == nil then return end          -- "cut" or unknown: no transition
    set("ScreenTransitionSettings.mode", mode)
    set("ScreenTransitionSettings.direction", transition_dir)
    set("ScreenTransitionSettings.smoothness", transition_soft)
    set("ScreenTransitionSettings.progress", 0.0)   -- < 1.0 freezes the snapshot
    _transitioning = true
    _t_clock = 0.0
end

function on_update()
    -- Where is the target right now?
    tx = get_on(target, "Transform.translation.x")
    ty = get_on(target, "Transform.translation.y")
    tz = get_on(target, "Transform.translation.z")
    if tx == nil or ty == nil or tz == nil then return end

    -- Advance the shot clock and the slow pan.
    _timer = _timer + delta
    _orbit = _orbit + (SHOTS[_shot].pan or 0.0) * delta

    advance = false
    if auto_cycle and _timer >= hold_time then advance = true end
    if is_key_just_pressed("KeyN")            then advance = true end
    if advance then
        _shot = _shot + 1
        if _shot > #SHOTS then _shot = 1 end
        _timer = 0.0
        _orbit = 0.0
        -- Begin the transition BEFORE repositioning: the snapshot freezes on the
        -- outgoing shot, then the camera cuts and the new shot is revealed.
        start_transition()
    end

    -- Ramp the transition: progress 0 -> 1 reveals the new shot over time.
    if _transitioning then
        _t_clock = _t_clock + delta
        local p = 1.0
        if transition_time > 0.0 then p = _t_clock / transition_time end
        if p >= 1.0 then
            p = 1.0
            _transitioning = false
        end
        set("ScreenTransitionSettings.progress", p)
    end

    -- Camera position for the current shot (orbiting around the target).
    -- The reposition is instant; the on-screen blend comes from the transition.
    shot = SHOTS[_shot]
    ang  = math.rad(shot.angle + _orbit)
    cam_x = tx + math.sin(ang) * shot.dist
    cam_y = ty + shot.height
    cam_z = tz + math.cos(ang) * shot.dist

    set_position(cam_x, cam_y, cam_z)
    look_at(tx, ty + look_height, tz)
end
