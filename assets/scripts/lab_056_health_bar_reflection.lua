-- lab_056_health_bar_reflection.lua
-- WHY: Demonstrates reflection-driven UI. We read/write Health.current straight
-- off the component by path and scale a child bar's X to the fraction, so the
-- bar stays in sync with whatever else mutates Health (damage, regen, scripts).
-- Setup: entity has a Health component (current/max); child named "Fill" is the bar.

function props()
  return {
    regen_per_sec = 3.0,
    bar_full_width = 1.0,
    _flash = 0.0,
  }
end

function on_update()
  local cur = get("Health.current")
  local max = get("Health.max")
  if cur == nil or max == nil or max <= 0.0 then return end

  -- Passive regen, clamped to max, written back through reflection.
  if cur < max then
    cur = clamp(cur + p.regen_per_sec * delta, 0.0, max)
    set("Health.current", cur)
  end

  -- Test hotkey to take a chunk of damage and flash the bar red.
  if is_key_just_pressed("KeyH") then
    cur = clamp(cur - 15.0, 0.0, max)
    set("Health.current", cur)
    p._flash = 0.3
  end

  -- Drive the fill bar width (anchored at left) from the fraction.
  local frac = cur / max
  set_child_scale("Fill", frac * p.bar_full_width, 1.0, 1.0)

  -- Color shifts green->red as health drops; flash overrides briefly.
  if p._flash > 0.0 then
    p._flash = p._flash - delta
    set_material_color(1.0, 0.1, 0.1)
  else
    set_material_color(1.0 - frac, frac, 0.1)
  end
end
