-- lab_037_ambient_loop.lua
-- Start a looping ambient bed once when the entity becomes ready.
-- WHY: play_sound_looping belongs in on_ready (not on_update) so the loop is
-- created exactly once; calling it per frame would stack endless copies.
-- Setup: set ambient to a seamless loop file (wind, hum, rain).

function props()
  return {
    ambient = { value = "audio/ambient_wind.ogg", hint = "Loop path", tab = "Audio" },
    volume = { value = 0.5, hint = "0..1", tab = "Audio" },
  }
end

function on_ready()
  play_sound_looping(ambient, volume)
end
