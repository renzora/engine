-- lab_041_toggle_light.lua
-- Toggle a named child light on/off with a key.
-- WHY: set_on writes a field on another entity by name, so we flip its
-- PointLight intensity between zero and the configured brightness.
-- Setup: set light_name to a light entity; press the toggle key.

local lit = true

function props()
  return {
    light_name = { value = "Lamp", hint = "Light entity name", tab = "Light" },
    toggle_key = { value = "KeyF", hint = "Toggle key", tab = "Light" },
    brightness = { value = 2000.0, hint = "On intensity", tab = "Light" },
  }
end

function on_update()
  if is_key_just_pressed(toggle_key) then
    lit = not lit
    local intensity = 0.0
    if lit then intensity = brightness end
    set_on(light_name, "PointLight.intensity", intensity)
  end
end
