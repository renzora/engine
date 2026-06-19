-- lab_067_healing_pickup.lua
-- WHY: A floating health pickup. It bobs + spins for readability, and when the
-- player is within pickup radius it adds to the player's Health via set_on
-- (clamped to max), plays a chime, then despawns itself so it can't double-heal.
-- Setup: place in the world; ensure a "Player" entity owns a Health component.

function props()
  return {
    heal = 25.0,
    radius = 1.5,
    bob_height = 0.25,
    spin_speed = 90.0,
    _base_y = 0.0,
    _claimed = false,
  }
end

function on_ready()
  p._base_y = position_y
end

function on_update()
  if p._claimed then return end

  -- Idle motion.
  set_position(position_x, p._base_y + math.sin(elapsed * 3.0) * p.bob_height, position_z)
  rotate(0.0, p.spin_speed * delta, 0.0)

  local px = get_on("Player", "Transform.translation.x")
  local pz = get_on("Player", "Transform.translation.z")
  if px == nil then return end

  local dx, dz = px - position_x, pz - position_z
  if (dx * dx + dz * dz) <= (p.radius * p.radius) then
    -- Apply the heal through reflection on the player, clamped to max.
    local cur = get_on("Player", "Health.current") or 0.0
    local max = get_on("Player", "Health.max") or cur
    set_on("Player", "Health.current", clamp(cur + p.heal, 0.0, max))
    play_sound("pickup_heal")
    p._claimed = true
    despawn_self()
  end
end
