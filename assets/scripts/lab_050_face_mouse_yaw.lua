-- lab_050_face_mouse_yaw.lua
-- Smoothly rotate to face a yaw derived from horizontal mouse movement.
-- WHY: we integrate mouse_delta_x into a target yaw, then lerp the current yaw
-- toward it so turning eases in instead of snapping; angle wrap is normalized.
-- Setup: move the mouse left/right; the entity turns to follow.

local cur_yaw = 0.0
local target_yaw = 0.0

function props()
  return {
    sensitivity = { value = 0.2, hint = "Degrees per mouse unit", tab = "Look" },
    smoothing = { value = 8.0, hint = "Higher = snappier turn", tab = "Look" },
  }
end

function on_ready()
  cur_yaw = rotation_y
  target_yaw = rotation_y
end

function on_update()
  target_yaw = target_yaw - mouse_delta_x * sensitivity

  -- Take the shortest path to the target so crossing +/-180 doesn't spin around.
  local diff = (target_yaw - cur_yaw)
  diff = (diff + 180.0) % 360.0 - 180.0

  local t = clamp(smoothing * delta, 0.0, 1.0)
  cur_yaw = cur_yaw + diff * t
  set_rotation(0.0, cur_yaw, 0.0)
end
