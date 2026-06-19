-- lab_036_footsteps.lua
-- Play a footstep sound at a fixed cadence while the entity is moving.
-- WHY: we accumulate distance traveled and emit a step every stride_length so
-- footstep rate scales with speed instead of with frame rate.
-- Setup: set step_sound to a valid audio path; pair with a movement script.

local last_x, last_z = 0.0, 0.0
local accum = 0.0

function props()
  return {
    step_sound = { value = "audio/footstep.ogg", hint = "Sound path", tab = "Audio" },
    stride_length = { value = 1.5, hint = "Distance per step", tab = "Audio" },
  }
end

function on_ready()
  last_x, last_z = position_x, position_z
end

function on_update()
  local dx = position_x - last_x
  local dz = position_z - last_z
  last_x, last_z = position_x, position_z

  accum = accum + math.sqrt(dx * dx + dz * dz)
  if accum >= stride_length then
    play_sound(step_sound, 0.6)
    accum = 0.0
  end
end
