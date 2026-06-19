-- lab_061_wave_spawner.lua
-- WHY: A timed wave director. Each wave is announced by a repeating timer; the
-- per-wave enemy count ramps so difficulty climbs. Spawns scatter inside a box
-- so they don't stack on one point. Prefix tagging keeps cleanup trivial.
-- Setup: place at the arena center; set spawn_name to a spawnable prefab/primitive.

function props()
  return {
    wave_interval = 8.0,
    base_count = 3,
    per_wave = 2,
    half_extent = 6.0,
    prefix = "wave_enemy",
    _wave = 0,
  }
end

function on_ready()
  start_timer("wave", p.wave_interval, true)
  spawn_wave()  -- kick off immediately, don't wait a full interval
end

function on_update()
  for _, name in ipairs(timers_finished) do
    if name == "wave" then spawn_wave() end
  end
end

function spawn_wave()
  p._wave = p._wave + 1
  local count = p.base_count + (p._wave - 1) * p.per_wave
  for i = 1, count do
    local x = position_x + (math.random() * 2.0 - 1.0) * p.half_extent
    local z = position_z + (math.random() * 2.0 - 1.0) * p.half_extent
    spawn_primitive(p.prefix, "cube", x, position_y + 0.5, z, 0.8, 0.2, 0.2)
  end
  play_sound("wave_start")
  print_log("wave " .. p._wave .. " -> " .. count .. " enemies")
end
