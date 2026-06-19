-- lab_026_topdown_move.lua
-- WASD top-down movement using a 2D input axis.
-- WHY: input_axis_2d gives a normalized stick/keyboard vector so the same code
-- works for keyboard and gamepad, keeping movement frame-rate independent via delta.
-- Setup: bind a "move" action to WASD (and a stick) in the input map.

function props()
  return {
    speed = { value = 5.0, hint = "Units per second", tab = "Movement" },
  }
end

function on_update()
  local ax, ay = input_axis_2d("move")
  if ax == 0 and ay == 0 then return end
  -- Top-down: X = strafe, Z = forward/back (screen up = -Z).
  translate(ax * speed * delta, 0.0, -ay * speed * delta)
end
