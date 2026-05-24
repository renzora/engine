-- Footsteps
--
-- Plays a footstep sound as the character moves, spaced by distance travelled
-- so the cadence naturally tracks how fast you're going. Steps are silenced
-- while airborne (jumping/falling) by reading the physics ground state.
--
-- Attach this to the CHARACTER entity (the one that moves). It needs no other
-- setup beyond a footstep audio file. If the character has a physics body, set
-- `require_grounded` so steps stop in mid-air; otherwise turn it off.
--
-- `clip` is relative to the project folder, e.g. "audio/footstep.mp3".

function props()
    return {
        clip            = { type = "string", value = "audio/footstep.mp3", hint = "Footstep sound, relative to the project folder" },
        bus             = { type = "string", value = "Sfx",   hint = "Mixer bus to play on" },
        volume          = { type = "float",  value = 0.6,     hint = "Base volume (0..2)" },
        volume_jitter   = { type = "float",  value = 0.15,    hint = "Random +/- volume per step, for variation" },
        step_distance   = { type = "float",  value = 1.5,     hint = "World units travelled between footsteps" },
        min_speed       = { type = "float",  value = 0.4,     hint = "Don't step below this speed (units/sec)" },
        require_grounded= { type = "bool",   value = true,    hint = "Only step while grounded (needs a physics body)" },

        -- Internal state
        _lx          = { type = "float", value = 0.0,   hint = "Internal: last x" },
        _lz          = { type = "float", value = 0.0,   hint = "Internal: last z" },
        _accum       = { type = "float", value = 0.0,   hint = "Internal: distance since last step" },
        _was_moving  = { type = "bool",  value = false, hint = "Internal: was moving last frame" },
        _init        = { type = "bool",  value = false, hint = "Internal: initialised" },
    }
end

function on_ready()
    _lx = position_x
    _lz = position_z
    _accum = 0.0
    _was_moving = false
    _init = true
end

-- Play one footstep with a little random volume variation.
local function step()
    local v = volume + (math.random() * 2.0 - 1.0) * volume_jitter
    if v < 0.0 then v = 0.0 end
    play_sound(clip, v, bus)
end

function on_update()
    if not _init then
        _lx, _lz, _accum, _was_moving, _init = position_x, position_z, 0.0, false, true
    end

    -- Horizontal distance moved this frame.
    local dx = position_x - _lx
    local dz = position_z - _lz
    _lx, _lz = position_x, position_z
    local dist = math.sqrt(dx * dx + dz * dz)
    local speed = (delta > 0.0) and (dist / delta) or 0.0

    -- Grounded check (physics body). Off -> always considered grounded.
    local grounded = true
    if require_grounded then
        grounded = get("PhysicsReadState.grounded") == true
    end

    local moving = grounded and speed >= min_speed
    if moving then
        if not _was_moving then
            -- Fire a step the instant you start moving, then space by distance.
            step()
            _accum = 0.0
        else
            _accum = _accum + dist
            if _accum >= step_distance then
                _accum = _accum - step_distance
                step()
            end
        end
    else
        -- Standing still or airborne: reset so the next stride starts clean.
        _accum = 0.0
    end
    _was_moving = moving
end
