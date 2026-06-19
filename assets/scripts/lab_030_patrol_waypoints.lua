-- lab_030_patrol_waypoints.lua
-- Patrol between waypoints stored as props, advancing on arrival.
-- WHY: keeping the path in props makes it designer-editable in the inspector;
-- we track the active index in a local and wrap around for an endless loop.
-- Setup: edit the waypoint coordinates; entity walks the loop forever.

local idx = 1
local waypoints = {
  { x = 0.0, z = 0.0 },
  { x = 5.0, z = 0.0 },
  { x = 5.0, z = 5.0 },
  { x = 0.0, z = 5.0 },
}

function props()
  return {
    speed = { value = 3.0, hint = "Units per second", tab = "Patrol" },
    arrive = { value = 0.2, hint = "Arrival radius", tab = "Patrol" },
  }
end

function on_update()
  local wp = waypoints[idx]
  local dx = wp.x - position_x
  local dz = wp.z - position_z
  local dist = math.sqrt(dx * dx + dz * dz)

  if dist <= arrive then
    -- Reached it: advance to the next waypoint (wrap to 1).
    idx = idx % #waypoints + 1
    return
  end

  translate((dx / dist) * speed * delta, 0.0, (dz / dist) * speed * delta)
end
