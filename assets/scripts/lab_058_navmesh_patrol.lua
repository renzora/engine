-- lab_058_navmesh_patrol.lua
-- WHY: Waypoint patrol on a baked navmesh. We hand the agent one destination at
-- a time and watch NavReadState to know when it arrives, then advance the index.
-- Letting the navmesh do pathfinding keeps the script free of steering math.
-- Setup: bake a navmesh; fill props.waypoints with {x,y,z} stops.

function props()
  return {
    waypoints = {
      { 5.0, 0.0, 0.0 },
      { 5.0, 0.0, 5.0 },
      { -5.0, 0.0, 5.0 },
      { -5.0, 0.0, -5.0 },
    },
    arrive_dist = 0.6,
    _index = 1,
    _sent = false,
  }
end

function on_ready()
  go_to_current()
end

function on_update()
  if not p._sent then return end

  -- Prefer the nav mirror's own arrival/remaining-distance if present.
  local remaining = get("NavReadState.remaining_distance")
  local arrived
  if remaining ~= nil then
    arrived = remaining <= p.arrive_dist
  else
    local wp = p.waypoints[p._index]
    local dx, dz = wp[1] - position_x, wp[3] - position_z
    arrived = (dx * dx + dz * dz) <= (p.arrive_dist * p.arrive_dist)
  end

  if arrived then
    p._index = p._index + 1
    if p._index > #p.waypoints then p._index = 1 end
    go_to_current()
  end
end

function go_to_current()
  local wp = p.waypoints[p._index]
  nav_set_destination(wp[1], wp[2], wp[3])
  p._sent = true
end
