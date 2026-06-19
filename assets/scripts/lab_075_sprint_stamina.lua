-- lab_075_sprint_stamina.lua
-- WHY: A sprint with a stamina budget. Holding shift while moving drains stamina
-- and boosts speed; releasing (or emptying the bar) regenerates it after a short
-- delay. Stamina persists in a prop and mirrors to a HUD label every change.
-- Setup: attach to a mover; UI label named "Stamina"; child "StaminaFill" is a bar.

function props()
  return {
    max_stamina = 100.0,
    stamina = 100.0,
    drain_per_sec = 35.0,
    regen_per_sec = 20.0,
    regen_delay = 1.0,
    walk_speed = 4.0,
    sprint_mult = 1.8,
    _regen_cd = 0.0,
    _shown = -1.0,
  }
end

function on_update()
  local mag = math.sqrt(input_x * input_x + input_y * input_y)
  local moving = mag > 0.05
  local wants_sprint = (is_key_pressed("ShiftLeft") or gamepad_l2 > 0.5) and moving and p.stamina > 0.0

  local speed = p.walk_speed
  if wants_sprint then
    speed = p.walk_speed * p.sprint_mult
    p.stamina = math.max(0.0, p.stamina - p.drain_per_sec * delta)
    p._regen_cd = p.regen_delay      -- block regen while spending
    set_anim_bool("Sprinting", true)
  else
    set_anim_bool("Sprinting", false)
    if p._regen_cd > 0.0 then
      p._regen_cd = p._regen_cd - delta
    elseif p.stamina < p.max_stamina then
      p.stamina = math.min(p.max_stamina, p.stamina + p.regen_per_sec * delta)
    end
  end

  -- Move camera-relative.
  local s, c = math.sin(camera_yaw), math.cos(camera_yaw)
  local wx = input_x * c + input_y * s
  local wz = -input_x * s + input_y * c
  translate(wx * speed * delta, 0.0, wz * speed * delta)

  -- Reflect stamina to the bar + label only when it changes meaningfully.
  local frac = p.stamina / p.max_stamina
  set_child_scale("StaminaFill", frac, 1.0, 1.0)
  local pct = math.floor(frac * 100.0 + 0.5)
  if pct ~= p._shown then
    p._shown = pct
    action("ui_set_text", { name = "Stamina", text = "Stamina " .. pct .. "%" })
  end
end
