-- lab_045_charge_meter.lua
-- Hold a key to charge a meter, release to "fire" with power scaled by charge.
-- WHY: we ramp a charge prop up while the key is held and act on release; the
-- visual color feedback (green->red) shows charge level without any UI.
-- Setup: hold charge_key to build power, release to launch.

local charging = false

function props()
  return {
    charge_key = { value = "Space", hint = "Hold to charge", tab = "Charge" },
    charge = { value = 0.0, hint = "0..1 current charge", tab = "Charge" },
    rate = { value = 0.8, hint = "Charge per second", tab = "Charge" },
  }
end

function on_update()
  if is_key_pressed(charge_key) then
    charging = true
    charge = clamp(charge + rate * delta, 0.0, 1.0)
    set_material_color(charge, 1.0 - charge, 0.0)   -- green -> red as it fills
  elseif charging then
    -- Released: fire with impulse proportional to stored charge, then reset.
    apply_impulse(0.0, 0.0, -charge * 12.0)
    screen_shake(charge * 0.5, 0.2)
    charge = 0.0
    charging = false
    set_material_color(0.0, 1.0, 0.0)
  end
end
