-- lab_094_camera_director.lua
-- Cinematic camera director: cycle through named presets on a timeline with
-- eased holds, like a cutscene sequencer.
-- WHY: a cutscene is just a scheduled list of (preset, dwell) pairs; the engine
-- has goto_camera_preset but no sequencer, so we own the shot index and a dwell
-- timer, advancing on each edge and looping. Setup: define presets in the
-- Camera inspector matching SHOTS; attach this to a director entity.
-- Multiplayer: local view direction; not networked.

local SHOTS = {
  { preset = "establishing", dwell = 4.0 },
  { preset = "closeup",      dwell = 2.5 },
  { preset = "over_shoulder",dwell = 3.0 },
  { preset = "wide_orbit",   dwell = 5.0 },
}
local idx = 0
local playing = false

local function cut_to(i)
  local s = SHOTS[i]
  if not s then return end
  goto_camera_preset(s.preset)
  start_timer("shot", s.dwell, false)
  print("[director] shot " .. i .. " -> " .. s.preset)
end

local function advance()
  idx = idx % #SHOTS + 1
  cut_to(idx)
end

function on_ready()
  playing = true
  idx = 1
  cut_to(idx)
end

function on_update()
  if not playing then return end
  for _, t in ipairs(timers_finished) do
    if t == "shot" then advance() end
  end
  -- Press a key to skip the current shot immediately.
  if input_skip then
    stop_timer("shot")
    advance()
  end
end
