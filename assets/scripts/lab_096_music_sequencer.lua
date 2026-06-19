-- lab_096_music_sequencer.lua
-- Generative step sequencer: a tempo clock walks patterns and triggers sounds,
-- building evolving music from one-shots.
-- WHY: there's no music-engine API, but a fixed-tempo timer + per-track step
-- masks let us sequence play_sound calls into a groove. Patterns mutate over
-- time (probabilistic fills) so it never loops identically. Setup: attach to a
-- manager; provide the referenced sfx. Multiplayer: local audio (gate to taste).

local BPM = 120
local STEPS = 16
local step = 0

-- Each track: a sound + a 16-step boolean mask + a trigger chance.
local tracks = {
  { snd = "sfx/kick.ogg",  mask = {true,false,false,false,true,false,false,false}, chance = 1.0 },
  { snd = "sfx/snare.ogg", mask = {false,false,true,false,false,false,true,false}, chance = 1.0 },
  { snd = "sfx/hat.ogg",   mask = {true,true,true,true,true,true,true,true},       chance = 0.8 },
}

local function maskbit(m, s) return m[(s % #m) + 1] end

function on_ready()
  local interval = 60.0 / BPM / 4.0  -- 16th notes
  start_timer("seq", interval, true)
end

function on_update()
  for _, t in ipairs(timers_finished) do
    if t == "seq" then
      for _, tr in ipairs(tracks) do
        if maskbit(tr.mask, step) and math.random() <= tr.chance then
          play_sound(tr.snd)
        end
      end
      -- Every bar, mutate the hat pattern for variation.
      if step % STEPS == 0 then
        local h = tracks[3].mask
        local i = math.random(#h)
        h[i] = not h[i]
      end
      step = (step + 1) % STEPS
    end
  end
end
