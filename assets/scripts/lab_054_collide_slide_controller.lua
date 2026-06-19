-- lab_054_collide_slide_controller.lua
-- WHY: A camera-relative character mover. move_controller() handles the actual
-- collide-and-slide against geometry; our job is to convert input + camera yaw
-- into a desired displacement and apply gravity so the body hugs the floor.
-- Setup: attach to a kinematic character body that owns a PhysicsReadState.

function props()
  return {
    move_speed = 5.0,
    gravity = -18.0,
    _vy = 0.0,
  }
end

function on_update()
  -- Rotate raw stick/keys into world space around the camera yaw.
  local s, c = math.sin(camera_yaw), math.cos(camera_yaw)
  local wx = input_x * c + input_y * s
  local wz = -input_x * s + input_y * c

  local grounded = get("PhysicsReadState.grounded")
  if grounded == nil then grounded = false end

  -- Keep a small downward bias while grounded so slopes stay seated.
  if grounded and p._vy < 0.0 then
    p._vy = -2.0
  else
    p._vy = p._vy + p.gravity * delta
  end

  local dx = wx * p.move_speed * delta
  local dz = wz * p.move_speed * delta
  local dy = p._vy * delta
  move_controller(dx, dy, dz)

  -- Face travel direction when actually moving.
  if math.abs(wx) + math.abs(wz) > 0.01 then
    look_at(position_x + wx, position_y, position_z + wz)
  end
end
