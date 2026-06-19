-- lab_072_day_night_orchestrator.lua
-- WHY: A single director that advances a 24h clock and drives sun angle, fog,
-- and music together so the whole scene stays in phase. Deriving everything from
-- one normalized time-of-day (0..1) means there is one source of truth to tune.
-- Setup: attach to an empty director entity; scene has a directional sun light.

function props()
  return {
    day_length = 120.0,    -- real seconds for a full cycle
    time_of_day = 0.25,    -- start at sunrise
    _last_phase = "",
  }
end

function on_update()
  -- Advance and wrap the normalized clock.
  p.time_of_day = (p.time_of_day + delta / p.day_length) % 1.0
  local t = p.time_of_day

  -- Sun elevation: a sine that peaks at noon (t=0.5), below horizon at night.
  local elevation = math.sin(t * math.pi * 2.0 - math.pi * 0.5) * 80.0
  local azimuth = t * 360.0
  set_sun_angles(elevation, azimuth)

  -- Day/dusk/night palette drives fog density + color.
  local daylight = clamp((elevation + 10.0) / 90.0, 0.0, 1.0)
  local fog_density = lerp(0.02, 0.006, daylight)        -- thicker at night
  set_fog(lerp(0.05, 0.6, daylight), lerp(0.06, 0.7, daylight), lerp(0.1, 0.85, daylight), fog_density)

  -- Swap the music bed on phase changes only (avoid restarting every frame).
  local phase
  if daylight > 0.6 then phase = "day"
  elseif daylight > 0.15 then phase = "dusk"
  else phase = "night" end
  if phase ~= p._last_phase then
    p._last_phase = phase
    play_music("ambient_" .. phase)
  end
end
