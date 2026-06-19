-- lab_029_follow_target.lua
-- Follow a named target entity by reading its Transform via get_on.
-- WHY: get_on("Name","Transform.translation") lets one entity track another
-- without a hard link; we stop short by keep_distance so we don't overlap it.
-- Setup: set target_name to the exact entity name to follow.

function props()
  return {
    target_name = { value = "Player", hint = "Entity name to follow", tab = "Follow" },
    speed = { value = 4.0, hint = "Units per second", tab = "Follow" },
    keep_distance = { value = 2.0, hint = "Stop this far away", tab = "Follow" },
  }
end

function on_update()
  local tp = get_on(target_name, "Transform.translation")
  if tp == nil then return end

  local dx = tp.x - position_x
  local dz = tp.z - position_z
  local dist = math.sqrt(dx * dx + dz * dz)
  if dist <= keep_distance or dist == 0 then return end

  -- Normalize and step toward the target.
  translate((dx / dist) * speed * delta, 0.0, (dz / dist) * speed * delta)
end
