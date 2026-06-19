-- lab_033_click_shake.lua
-- Trigger a screen shake on left mouse click.
-- WHY: screen_shake is a one-shot effect, so we fire it only on the
-- just-pressed edge (mouse_left_just_pressed) to avoid retriggering every frame.
-- Setup: attach to any entity; intensity/duration tunable in the inspector.

function props()
  return {
    intensity = { value = 0.4, hint = "Shake strength", tab = "FX" },
    duration = { value = 0.25, hint = "Seconds", tab = "FX" },
  }
end

function on_update()
  if mouse_left_just_pressed then
    screen_shake(intensity, duration)
  end
end
