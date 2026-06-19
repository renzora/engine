-- lab_024_hue_cycle: cycles the material through the full hue wheel.
-- set_material_color only takes RGB, so we do a tiny inline HSV->RGB at full
-- saturation/value; phase-shifting one sine by 120 degrees per channel is the
-- cheapest way to sweep hue without a lookup table.

function props()
    return {
        speed = { value = 0.2, hint = "Hue revolutions per second", tab = "Hue" },
    }
end

local function channel(phase)
    -- sine offset into 0..1, biased so the three channels peak 1/3 apart.
    return clamp(math.sin(phase) * 0.5 + 0.5, 0.0, 1.0)
end

function on_update()
    local a = elapsed * speed * math.pi * 2.0
    local third = math.pi * 2.0 / 3.0
    set_material_color(channel(a), channel(a - third), channel(a - third * 2.0))
end
