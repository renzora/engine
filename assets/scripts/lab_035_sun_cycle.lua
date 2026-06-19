-- lab_035_sun_cycle.lua
-- Day/night cycle: animate the sun angle from elapsed time.
-- WHY: elapsed is monotonic engine time, so deriving the angle from it (rather
-- than accumulating) is drift-free and resumes correctly after a pause.
-- Setup: scene needs a directional/sun light driven by set_sun_angles.

function props()
  return {
    day_length = { value = 60.0, hint = "Seconds per full day", tab = "Sky" },
  }
end

function on_update()
  -- One full 360 deg revolution per day_length; elevation rides a sine so the
  -- sun rises, peaks overhead, and sets below the horizon.
  local frac = (elapsed % day_length) / day_length
  local azimuth = frac * 360.0
  local elevation = math.sin(frac * math.pi * 2.0) * 80.0
  set_sun_angles(azimuth, elevation)
end
