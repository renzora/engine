-- Light Flicker
--
-- Flickers a light's intensity over time for torches, campfires, faulty bulbs,
-- etc. Attach this to the LIGHT entity (a Point Light or Spot Light). It reads
-- and writes the light's `intensity` via reflection, so no engine support is
-- needed — it's the same approach Unity/Godot use (a script animating the
-- light's intensity).
--
-- NOTE: scripts only run in PLAY mode — you won't see flicker while editing.
--
-- `component` defaults to "auto", which detects whether this entity has a
-- PointLight or SpotLight. Set it explicitly if you want to force one.
--
-- Styles:
--   "campfire"- lively fire: slow swell + fast flicker + random flares/dips
--   "torch"   - organic smooth flicker (layered sines)  -> steady torch
--   "pulse"   - gentle sine breathing                   -> magic / sci-fi
--   "flicker" - sharp random with occasional deep dips  -> broken / buzzing bulb

function props()
    return {
        component      = { type = "string", value = "auto", hint = "auto | PointLight | SpotLight" },
        style          = { type = "string", value = "campfire", hint = "campfire | torch | pulse | flicker" },
        amount         = { type = "float",  value = 0.5,  hint = "Flicker depth 0..1 (how far intensity dips). Try 0.5 campfire, 0.8 broken bulb" },
        speed          = { type = "float",  value = 12.0, hint = "Flicker speed" },
        min_factor     = { type = "float",  value = 0.2,  hint = "Lower clamp on the intensity multiplier (avoids fully dark)" },
        brightness     = { type = "float",  value = 1.0,  hint = "Overall intensity multiplier — raise for a much brighter fire" },
        base_intensity = { type = "float",  value = 0.0,  hint = "Base intensity; 0 = capture the light's current value on start" },
        range_min      = { type = "float",  value = 4.5,  hint = "Min light range during dips (0 with range_max=0 to leave range alone)" },
        range_max      = { type = "float",  value = 6.0,  hint = "Max light range during flares (0 = don't animate range)" },

        -- Internal state
        _comp  = { type = "string", value = "", hint = "Internal: resolved light component" },
        _base  = { type = "float", value = 0.0, hint = "Internal: base intensity" },
        _t    = { type = "float", value = 0.0, hint = "Internal: flicker time" },
        _rt   = { type = "float", value = 0.0, hint = "Internal: random hold timer (flicker style)" },
        _cur  = { type = "float", value = 1.0, hint = "Internal: current random level" },
        _tgt  = { type = "float", value = 1.0, hint = "Internal: target random level" },
        _init = { type = "bool",  value = false, hint = "Internal: initialised" },
    }
end

-- Find the light component on this entity that actually has an `intensity`.
local function resolve_component()
    -- Honour an explicit choice if it exists on this entity.
    if component ~= "" and component ~= "auto" then
        if type(get(component .. ".intensity")) == "number" then
            return component
        end
    end
    -- Otherwise auto-detect.
    for _, name in ipairs({ "PointLight", "SpotLight" }) do
        if type(get(name .. ".intensity")) == "number" then
            return name
        end
    end
    return ""
end

-- Capture the light's authored values to animate around.
local function capture_base()
    if base_intensity > 0.0 then
        _base = base_intensity
    elseif _comp ~= "" then
        local v = get(_comp .. ".intensity")
        _base = (type(v) == "number" and v > 0.0) and v or 1000.0
    else
        _base = 1000.0
    end
end

function on_ready()
    _comp = resolve_component()
    capture_base()
    _t, _rt, _cur, _tgt, _init = 0.0, 0.0, 1.0, 1.0, true
end

function on_update()
    if not _init or _comp == "" then
        _comp = resolve_component()
        if _comp == "" then return end
        capture_base()
        _t, _rt, _cur, _tgt, _init = 0.0, 0.0, 1.0, 1.0, true
    end

    _t = _t + delta * speed

    -- Noise in 0..1 depending on style.
    local n
    if style == "campfire" then
        -- Busy fire: mostly fast flicker + frequent random flares/dips, with
        -- just a hint of slow swell underneath.
        local slow = 0.5 + 0.5 * math.sin(_t * 0.5)
        local fast = 0.5 + 0.5 * (0.6 * math.sin(_t) + 0.4 * math.sin(_t * 2.9 + 1.3))
        _rt = _rt - delta * speed
        if _rt <= 0.0 then
            _tgt = math.random()
            _rt = 0.06 + math.random() * 0.22
        end
        _cur = _cur + (_tgt - _cur) * math.min(1.0, delta * speed * 6.0)
        n = 0.45 * fast + 0.15 * slow + 0.40 * _cur
    elseif style == "pulse" then
        n = 0.5 + 0.5 * math.sin(_t)
    elseif style == "flicker" then
        _rt = _rt - delta * speed
        if _rt <= 0.0 then
            _tgt = math.random()
            if math.random() < 0.2 then _tgt = _tgt * 0.15 end
            _rt = 0.4 + math.random() * 0.8
        end
        _cur = _cur + (_tgt - _cur) * math.min(1.0, delta * speed * 3.0)
        n = _cur
    else
        -- "torch": layered sines at incommensurate rates = organic flicker.
        n = 0.5 + 0.5 * (0.55 * math.sin(_t)
                        + 0.30 * math.sin(_t * 2.3 + 1.7)
                        + 0.15 * math.sin(_t * 5.1 + 4.2))
    end

    -- Intensity multiplier. Campfire is bipolar (dips AND flares above base);
    -- the other styles only dim from base.
    local m
    if style == "campfire" then
        m = 1.0 + amount * (n * 2.0 - 1.0)
    else
        m = (1.0 - amount) + amount * n
    end
    if m < min_factor then m = min_factor end

    set(_comp .. ".intensity", _base * brightness * m)

    -- Range pulses with the flame: farther reach on flares, shorter on dips.
    if range_max > 0.0 then
        set(_comp .. ".range", range_min + (range_max - range_min) * n)
    end
end
