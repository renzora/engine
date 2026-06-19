-- lab_031_smooth_follow.lua
-- Smooth follow using lerp toward a named target's position.
-- WHY: lerping toward the target each frame (instead of snapping) gives the
-- springy "camera lag" feel; a higher smoothing value reacts faster.
-- Setup: set target_name; attach to a camera rig or companion entity.

function props()
  return {
    target_name = { value = "Player", hint = "Entity to follow", tab = "Follow" },
    smoothing = { value = 5.0, hint = "Higher = snappier", tab = "Follow" },
    offset_y = { value = 3.0, hint = "Height above target", tab = "Follow" },
  }
end

function on_update()
  local tp = get_on(target_name, "Transform.translation")
  if tp == nil then return end

  -- delta-scaled lerp factor keeps the smoothing frame-rate independent.
  local t = clamp(smoothing * delta, 0.0, 1.0)
  local nx = lerp(position_x, tp.x, t)
  local ny = lerp(position_y, tp.y + offset_y, t)
  local nz = lerp(position_z, tp.z, t)
  set_position(nx, ny, nz)
end
