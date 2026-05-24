-- Footsteps
--
-- Fires footstep sounds as the character moves, spaced by distance travelled so
-- the cadence tracks how fast you're going. Steps are silenced while airborne
-- (jumping/falling) by reading the physics ground state.
--
-- This script only decides *when* a step happens — the actual sounds live on an
-- AudioPlayer component. Attach this to the CHARACTER entity, and give that same
-- entity an **Audio Player** component with a **Clip Pool** of footstep sounds
-- (drag several files in). Each step calls play_audio(), which picks a random
-- clip from the pool with per-shot pitch/volume jitter.
--
-- "Moving" is gated on a *smoothed* speed, not raw per-frame distance: the
-- smoothing rides over the physics-vs-render framerate mismatch (no stutter),
-- and the `min_speed` threshold ignores the tiny position drift the physics
-- solver produces while the character settles after you stop (no phantom steps).
-- A `min_interval` cooldown caps the rate so clips never pile up.

function props()
    return {
        step_distance    = { type = "float", value = 1.42, hint = "World units travelled between footsteps (smaller = faster cadence / jog)" },
        min_interval     = { type = "float", value = 0.38, hint = "Minimum seconds between footsteps; caps the fastest cadence (mainly limits sprint)" },
        min_speed        = { type = "float", value = 1.0,  hint = "Don't step below this speed (units/sec); also rejects settle-drift" },
        require_grounded = { type = "bool",  value = true, hint = "Only step while grounded (needs a physics body)" },

        -- Internal state
        _lx          = { type = "float", value = 0.0, hint = "Internal: last x" },
        _lz          = { type = "float", value = 0.0, hint = "Internal: last z" },
        _spd         = { type = "float", value = 0.0, hint = "Internal: smoothed horizontal speed" },
        _accum       = { type = "float", value = 0.0, hint = "Internal: distance since last step" },
        _since        = { type = "float", value = 0.0, hint = "Internal: time since last step" },
        _was_moving  = { type = "bool",  value = false, hint = "Internal: was moving last frame" },
        _init        = { type = "bool",  value = false, hint = "Internal: initialised" },
    }
end

function on_ready()
    _lx = position_x
    _lz = position_z
    _spd = 0.0
    _accum = 0.0
    _since = 0.0
    _was_moving = false
    _init = true
end

function on_update()
    if not _init then
        _lx, _lz, _spd, _accum, _since, _was_moving, _init =
            position_x, position_z, 0.0, 0.0, 0.0, false, true
    end
    if delta <= 0.0 then return end

    -- Horizontal distance/speed this frame (vertical motion is ignored so
    -- gravity / ground-stick never counts as walking).
    local dx = position_x - _lx
    local dz = position_z - _lz
    _lx, _lz = position_x, position_z
    local dist = math.sqrt(dx * dx + dz * dz)
    local speed = dist / delta

    -- Smooth the speed (~0.1s time constant) to ride over frames where physics
    -- didn't tick. Decays quickly to ~0 once you actually stop.
    _spd = _spd + (speed - _spd) * math.min(1.0, delta * 10.0)
    _since = _since + delta

    local grounded = true
    if require_grounded then
        grounded = get("PhysicsReadState.grounded") == true
    end

    local moving = grounded and _spd >= min_speed
    if moving then
        if not _was_moving then
            -- Just started: step right away so there's no lag on the first step.
            if _since >= min_interval then
                play_audio()
                _since = 0.0
            end
            _accum = 0.0
        else
            _accum = _accum + dist
            -- Step once we've covered a stride, but never faster than min_interval.
            if _accum >= step_distance and _since >= min_interval then
                play_audio()
                _accum = 0.0
                _since = 0.0
            end
        end
    else
        _accum = 0.0
    end
    _was_moving = moving
end
