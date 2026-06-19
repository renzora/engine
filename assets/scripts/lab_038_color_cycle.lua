-- lab_038_color_cycle.lua
-- Cycle the material through three colors on each key press.
-- WHY: we advance an index only on the just-pressed edge so one tap = one step;
-- a palette table keeps the colors data-driven and easy to extend.
-- Setup: entity needs a material; press the cycle key to advance.

local idx = 1
local palette = {
  { r = 0.9, g = 0.2, b = 0.2 },
  { r = 0.2, g = 0.8, b = 0.3 },
  { r = 0.2, g = 0.4, b = 0.9 },
}

function props()
  return {
    cycle_key = { value = "KeyC", hint = "Key to advance color", tab = "Color" },
  }
end

function on_ready()
  local c = palette[idx]
  set_material_color(c.r, c.g, c.b)
end

function on_update()
  if is_key_just_pressed(cycle_key) then
    idx = idx % #palette + 1
    local c = palette[idx]
    set_material_color(c.r, c.g, c.b)
  end
end
