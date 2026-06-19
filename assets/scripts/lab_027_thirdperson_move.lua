-- lab_027_thirdperson_move.lua
-- Camera-relative third-person movement.
-- WHY: raw WASD feels wrong in 3D; we rotate the input vector by camera_yaw so
-- "forward" always means "away from the camera", the standard TPS expectation.
-- Setup: bind a "move" action to WASD; works with the orbit/follow camera.

function props()
  return {
    speed = { value = 6.0, hint = "Units per second", tab = "Movement" },
  }
end

function on_update()
  local ax, ay = input_axis_2d("move")
  if ax == 0 and ay == 0 then return end

  -- Rotate the (ax, ay) input by the camera yaw so motion is camera-relative.
  local s, c = math.sin(camera_yaw), math.cos(camera_yaw)
  local fwd_x, fwd_z =  s,  c   -- camera forward on the XZ plane
  local right_x, right_z = c, -s

  local dx = (right_x * ax + fwd_x * ay) * speed * delta
  local dz = (right_z * ax + fwd_z * ay) * speed * delta
  translate(dx, 0.0, dz)
end
