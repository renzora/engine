-- lab_042_sprint_modifier.lua
-- WASD movement with a Shift sprint modifier.
-- WHY: holding Shift multiplies the base speed; checking is_key_pressed (held,
-- not just-pressed) means sprint stays active for as long as the key is down.
-- Setup: bind a "move" action to WASD; hold Left Shift to sprint.

function props()
  return {
    walk_speed = { value = 4.0, hint = "Base speed", tab = "Movement" },
    sprint_mult = { value = 2.0, hint = "Sprint multiplier", tab = "Movement" },
  }
end

function on_update()
  local ax, ay = input_axis_2d("move")
  if ax == 0 and ay == 0 then return end

  local speed = walk_speed
  if is_key_pressed("ShiftLeft") then
    speed = speed * sprint_mult
  end
  translate(ax * speed * delta, 0.0, -ay * speed * delta)
end
