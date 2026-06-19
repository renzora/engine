-- lab_060_projectile_tracker.lua
-- WHY: A fire-and-forget shooter. Each shot is a spawned primitive sharing the
-- "bolt" prefix; we don't keep per-bolt handles. Instead a global lifetime timer
-- periodically clears the whole prefix, so old bolts can't leak forever.
-- Setup: attach to a muzzle entity; press F (or gamepad R2) to fire.

function props()
  return {
    fire_cooldown = 0.18,
    sweep_interval = 3.0,   -- how often we cull old bolts
    prefix = "bolt",
    _cd = 0.0,
  }
end

function on_ready()
  -- Repeating timer drives the periodic cleanup sweep.
  start_timer("bolt_sweep", p.sweep_interval, true)
end

function on_update()
  if p._cd > 0.0 then p._cd = p._cd - delta end

  local firing = is_key_pressed("KeyF") or gamepad_r2 > 0.5
  if firing and p._cd <= 0.0 then
    fire()
    p._cd = p.fire_cooldown
  end

  -- When the sweep timer ticks, recycle every live bolt at once.
  for _, name in ipairs(timers_finished) do
    if name == "bolt_sweep" then
      despawn_by_prefix(p.prefix)
    end
  end
end

function fire()
  -- Spawn slightly in front of the muzzle along its facing.
  local s, c = math.sin(math.rad(rotation_y)), math.cos(math.rad(rotation_y))
  local x = position_x + s * 0.8
  local z = position_z + c * 0.8
  spawn_primitive(p.prefix, "sphere", x, position_y, z, 1.0, 0.8, 0.2)
  play_sound("shoot")
end
