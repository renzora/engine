-- lab_062_twin_stick.lua
-- WHY: Twin-stick control: left stick moves on the XZ plane, right stick aims
-- the body independently of travel. We only re-aim when the right stick is past
-- a deadzone, otherwise the body keeps its last facing (no snap-to-zero spin).
-- Setup: attach to a top-down character; a connected gamepad drives both sticks.

function props()
  return {
    move_speed = 6.0,
    deadzone = 0.25,
    fire_prefix = "ts_bolt",
    _cd = 0.0,
  }
end

function on_update()
  if p._cd > 0.0 then p._cd = p._cd - delta end

  -- Move on left stick.
  local mx, my = gamepad_left_x, gamepad_left_y
  if math.abs(mx) < p.deadzone then mx = 0.0 end
  if math.abs(my) < p.deadzone then my = 0.0 end
  translate(mx * p.move_speed * delta, 0.0, my * p.move_speed * delta)

  -- Aim on right stick (only when pushed past the deadzone).
  local ax, ay = gamepad_right_x, gamepad_right_y
  local amag = math.sqrt(ax * ax + ay * ay)
  if amag > p.deadzone then
    local yaw = math.deg(math.atan(ax, ay))
    set_rotation(0.0, yaw, 0.0)
    -- Fire while aiming and holding the right trigger.
    if gamepad_r2 > 0.5 and p._cd <= 0.0 then
      spawn_primitive(p.fire_prefix, "sphere",
        position_x + (ax / amag) * 0.8, position_y, position_z + (ay / amag) * 0.8,
        1.0, 0.9, 0.3)
      p._cd = 0.15
    end
  end
end
