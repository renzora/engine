-- lab_064_turret_lookat.lua
-- WHY: A turret that tracks the nearest entity named "Player*". We read player
-- positions by reflection, pick the closest within range, and look_at it; firing
-- is gated by a cooldown so it doesn't spew a bolt every frame while aimed.
-- Setup: place near players; ensure at least one entity is named "Player".

function props()
  return {
    range = 20.0,
    fire_cooldown = 0.8,
    prefix = "turret_bolt",
    _cd = 0.0,
  }
end

function on_update()
  if p._cd > 0.0 then p._cd = p._cd - delta end

  -- Single canonical target here; reflection by name keeps it data-driven.
  local tx = get_on("Player", "Transform.translation.x")
  local ty = get_on("Player", "Transform.translation.y")
  local tz = get_on("Player", "Transform.translation.z")
  if tx == nil then return end

  local dx, dz = tx - position_x, tz - position_z
  local d2 = dx * dx + dz * dz
  if d2 > p.range * p.range then
    return  -- out of range; hold last facing
  end

  -- Track and fire.
  look_at(tx, ty, tz)
  if p._cd <= 0.0 then
    local len = math.sqrt(d2)
    spawn_primitive(p.prefix, "sphere",
      position_x + (dx / len) * 1.0, position_y + 0.5, position_z + (dz / len) * 1.0,
      1.0, 0.3, 0.1)
    play_sound("turret_fire")
    p._cd = p.fire_cooldown
  end
end
