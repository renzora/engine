-- lab_040_ability_cooldown.lua
-- Fire an ability on key press, gated by a cooldown timer.
-- WHY: we use a named engine timer and watch timers_finished for its name so we
-- never double-fire; the local 'ready' flag blocks input until it elapses.
-- Setup: bind ability_key; cooldown is in seconds.

local ready = true

function props()
  return {
    ability_key = { value = "KeyQ", hint = "Activation key", tab = "Ability" },
    cooldown = { value = 2.0, hint = "Seconds between uses", tab = "Ability" },
  }
end

function on_update()
  -- The timer firing means the cooldown elapsed; re-arm the ability.
  for _, name in ipairs(timers_finished) do
    if name == "ability_cd" then ready = true end
  end

  if ready and is_key_just_pressed(ability_key) then
    screen_shake(0.3, 0.15)          -- stand-in for the actual ability effect
    play_sound("audio/cast.ogg", 0.7)
    ready = false
    start_timer("ability_cd", cooldown, false)
  end
end
