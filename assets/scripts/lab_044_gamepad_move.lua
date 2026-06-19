-- lab_044_gamepad_move.lua
-- Move with the left analog stick, including a deadzone.
-- WHY: sticks rest slightly off-center, so we zero out small magnitudes
-- (deadzone) to stop drift; movement scales with how far the stick is pushed.
-- Setup: connect a gamepad; left stick drives planar movement.

function props()
  return {
    speed = { value = 6.0, hint = "Max units/sec", tab = "Movement" },
    deadzone = { value = 0.15, hint = "Ignore below this", tab = "Movement" },
  }
end

function on_update()
  local x, y = gamepad_left_x, gamepad_left_y
  local mag = math.sqrt(x * x + y * y)
  if mag < deadzone then return end

  -- Stick up (positive Y) should move forward = -Z.
  translate(x * speed * delta, 0.0, -y * speed * delta)
end
