-- lab_048_respawn.lua
-- Snap back to the spawn point when the respawn key is pressed.
-- WHY: we capture the starting position in on_ready (the authoritative origin)
-- and restore it on key; zeroing velocity prevents carrying momentum into spawn.
-- Setup: press respawn_key to teleport home.

local home_x, home_y, home_z = 0.0, 0.0, 0.0

function props()
  return {
    respawn_key = { value = "KeyR", hint = "Respawn key", tab = "Respawn" },
  }
end

function on_ready()
  home_x, home_y, home_z = position_x, position_y, position_z
end

function on_update()
  if is_key_just_pressed(respawn_key) then
    set_velocity(0.0, 0.0, 0.0)
    set_position(home_x, home_y, home_z)
  end
end
