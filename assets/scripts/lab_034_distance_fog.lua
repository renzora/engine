-- lab_034_distance_fog.lua
-- Drive fog start/end from the entity's distance to the world origin.
-- WHY: pulling fog in as the player approaches a landmark is a cheap way to
-- build dread/focus; we only call set_fog when the band actually changes.
-- Setup: attach to the player; tune near/far multipliers.

local last_start = -1.0

function props()
  return {
    near = { value = 5.0, hint = "Fog start at min distance", tab = "Fog" },
    far = { value = 40.0, hint = "Fog end at min distance", tab = "Fog" },
  }
end

function on_update()
  local dist = math.sqrt(position_x * position_x + position_z * position_z)
  -- Farther from origin => fog closes in (start shrinks toward the player).
  local start = clamp(near - dist * 0.1, 0.5, near)
  if math.abs(start - last_start) > 0.05 then
    set_fog(true, start, far)
    last_start = start
  end
end
