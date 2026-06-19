-- lab_028_mouse_look.lua
-- Mouse-look that accumulates pitch/yaw from raw mouse deltas.
-- WHY: we keep our own yaw/pitch state and clamp pitch so the view can't flip
-- over the poles; set_rotation each frame from the accumulator (degrees).
-- Setup: attach to a camera or head entity; cursor is locked on ready.

local yaw = 0.0
local pitch = 0.0

function props()
  return {
    sensitivity = { value = 0.15, hint = "Degrees per mouse unit", tab = "Look" },
  }
end

function on_ready()
  yaw = rotation_y
  pitch = rotation_x
  lock_cursor()
end

function on_update()
  yaw = yaw - mouse_delta_x * sensitivity
  pitch = clamp(pitch - mouse_delta_y * sensitivity, -89.0, 89.0)
  set_rotation(pitch, yaw, 0.0)
end
