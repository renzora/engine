-- lab_091_daynight_weather.lua
-- World orchestrator: drives a day/night clock, sun angle, fog, music cues and
-- broadcasts the canonical time so all peers share one sky.
-- WHY: ambience is several coupled systems (sun, fog, audio) keyed off one
-- clock; centralizing it avoids drift. The server owns time and rpc's it; a
-- weather FSM swaps fog density and triggers a music change on transitions.
-- Setup: attach to a single world manager entity.
-- Multiplayer: server broadcasts "world_time"; clients render, never advance.

local DAY_LEN = 120.0     -- seconds per full day
local time_of_day = 6.0   -- hours (0..24)
local weather = "clear"
local weather_timer = 0
local last_track = ""

local function apply_sky()
  -- Sun elevation: -90 at midnight, +90 at noon.
  local elev = math.sin((time_of_day / 24.0) * math.pi * 2 - math.pi / 2) * 90
  set_sun_angles(elev, 45)

  -- Night and storms thicken fog; daytime clear is crisp.
  local night = (time_of_day < 6 or time_of_day > 19)
  local density = night and 0.06 or 0.01
  if weather == "storm" then density = density + 0.08 end
  set_fog(0.5, 0.55, 0.65, density)

  -- Music selection driven by time + weather, only restart on change.
  local track = weather == "storm" and "music/storm.ogg"
    or (night and "music/night.ogg" or "music/day.ogg")
  if track ~= last_track then
    last_track = track
    play_music(track)
  end
end

function on_ready()
  if net_is_server() or not net_is_connected() then apply_sky() end
end

function on_update()
  if net_is_client() and not net_is_server() then return end

  time_of_day = (time_of_day + (24.0 / DAY_LEN) * delta) % 24.0

  -- Weather FSM: every ~20s, roll for a transition.
  weather_timer = weather_timer + delta
  if weather_timer > 20.0 then
    weather_timer = 0
    weather = (math.random() < 0.3) and "storm" or "clear"
    if weather == "storm" then screen_shake(0.3, 1.0) end
  end

  apply_sky()
  if net_is_connected() then
    rpc("world_time", { t = time_of_day, w = weather })
  end
end

function on_rpc(name, args, from)
  if name == "world_time" and net_is_client() then
    time_of_day = args.t
    weather = args.w
    apply_sky()
  end
end
