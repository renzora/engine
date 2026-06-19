-- lab_047_proximity_trigger.lua
-- Fire a one-shot reaction the moment something starts colliding with this entity.
-- WHY: is_colliding is a level (true every frame while overlapping), so we keep
-- the previous state and act only on the rising edge to avoid spamming the effect.
-- Setup: entity needs a sensor/collider; reacts on first contact.

local was_colliding = false

function props()
  return {
    sound = { value = "audio/trigger.ogg", hint = "Sound on enter", tab = "Trigger" },
  }
end

function on_update()
  if is_colliding and not was_colliding then
    -- Rising edge: just entered the trigger volume.
    play_sound(sound, 0.8)
    set_material_color(1.0, 0.85, 0.2)
    print_log("Proximity trigger entered: " .. self_entity_name)
  elseif not is_colliding and was_colliding then
    set_material_color(0.6, 0.6, 0.6)
  end
  was_colliding = is_colliding
end
