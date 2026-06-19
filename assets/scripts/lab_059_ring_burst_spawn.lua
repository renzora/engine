-- lab_059_ring_burst_spawn.lua
-- WHY: Spawns a ring of primitives in one burst. Useful for shockwave debris or
-- a spawn-point telegraph. We tag every spawn with a shared prefix so a single
-- despawn_by_prefix can clear the whole ring later without tracking handles.
-- Setup: press B to fire a burst around this entity.

function props()
  return {
    count = 12,
    radius = 3.0,
    prefix = "ring_chunk",
    _burst = 0,
  }
end

function on_update()
  if is_key_just_pressed("KeyB") then
    spawn_ring()
  end
  -- Press C to clear all previously spawned chunks.
  if is_key_just_pressed("KeyC") then
    despawn_by_prefix(p.prefix)
  end
end

function spawn_ring()
  p._burst = p._burst + 1
  for i = 0, p.count - 1 do
    local a = (i / p.count) * math.pi * 2.0
    local x = position_x + math.cos(a) * p.radius
    local z = position_z + math.sin(a) * p.radius
    -- Color cycles around the ring so the burst reads as a rainbow wheel.
    local hue = i / p.count
    spawn_primitive(p.prefix, "sphere", x, position_y + 0.5, z, hue, 1.0 - hue, 0.5)
  end
  play_sound("spawn")
end
