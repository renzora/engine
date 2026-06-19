-- lab_043_strafe_turn.lua
-- Tank-style controls: forward/back drives, left/right turns the body.
-- WHY: input_y moves along local forward and input_x rotates yaw, so movement
-- follows facing; we recompute the forward vector from our own yaw each frame.
-- Setup: uses the built-in input_x / input_y axes (WASD / arrows).

local yaw = 0.0

function props()
  return {
    move_speed = { value = 5.0, hint = "Forward units/sec", tab = "Movement" },
    turn_speed = { value = 120.0, hint = "Degrees/sec", tab = "Movement" },
  }
end

function on_ready()
  yaw = rotation_y
end

function on_update()
  -- Turn first, then drive along the new facing.
  yaw = yaw - input_x * turn_speed * delta
  set_rotation(0.0, yaw, 0.0)

  if input_y ~= 0 then
    local rad = math.rad(yaw)
    local fwd_x, fwd_z = math.sin(rad), math.cos(rad)
    local d = input_y * move_speed * delta
    translate(fwd_x * d, 0.0, fwd_z * d)
  end
end
