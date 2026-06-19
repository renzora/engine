-- lab_071_ability_combo.lua
-- WHY: A three-button ability bar with per-ability cooldowns plus a combo window.
-- Pressing abilities in the right order inside the window triggers a finisher.
-- Cooldowns are independent timers tracked as floats so the UI can read them.
-- Setup: attach to the player; abilities map to keys Q/W/E.

function props()
  return {
    cd_q = 0.0, cd_w = 0.0, cd_e = 0.0,
    cost_q = 1.5, cost_w = 2.5, cost_e = 4.0,
    combo_window = 1.0,
    _combo = "",
    _combo_t = 0.0,
  }
end

function on_update()
  -- Tick down every cooldown.
  p.cd_q = math.max(0.0, p.cd_q - delta)
  p.cd_w = math.max(0.0, p.cd_w - delta)
  p.cd_e = math.max(0.0, p.cd_e - delta)

  -- Combo buffer decays; reset the recorded sequence when it lapses.
  if p._combo_t > 0.0 then
    p._combo_t = p._combo_t - delta
    if p._combo_t <= 0.0 then p._combo = "" end
  end

  if is_key_just_pressed("KeyQ") and p.cd_q <= 0.0 then cast("Q", p.cost_q) end
  if is_key_just_pressed("KeyW") and p.cd_w <= 0.0 then cast("W", p.cost_w) end
  if is_key_just_pressed("KeyE") and p.cd_e <= 0.0 then cast("E", p.cost_e) end
end

function cast(key, cost)
  if key == "Q" then p.cd_q = cost; trigger_anim("CastQ") end
  if key == "W" then p.cd_w = cost; trigger_anim("CastW") end
  if key == "E" then p.cd_e = cost; trigger_anim("CastE") end
  play_sound("cast")

  -- Append to the combo and re-arm the window.
  p._combo = p._combo .. key
  p._combo_t = p.combo_window
  if #p._combo > 3 then p._combo = string.sub(p._combo, -3) end

  -- The signature finisher.
  if p._combo == "QWE" then
    trigger_anim("Finisher")
    play_sound("finisher")
    screen_shake(0.5, 0.4)
    p._combo = ""
  end
end
