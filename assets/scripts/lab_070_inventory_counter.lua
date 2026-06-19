-- lab_070_inventory_counter.lua
-- WHY: A persisted pickup counter. The count lives in a prop so it survives
-- script reloads and is editor-visible, and every change pushes the new value to
-- a HUD label via the ui_set_text action. Picking up uses proximity, not events.
-- Setup: spawn coins with name prefix "coin_"; a UI label is named "CoinCount".

function props()
  return {
    coins = 0,
    goal = 10,
    pickup_radius = 1.2,
    _won = false,
  }
end

function on_ready()
  refresh_label()
end

function on_update()
  if p._won then return end

  -- A debug key adds a coin so the counter is testable without world coins.
  if is_key_just_pressed("KeyK") then
    add_coins(1)
  end

  -- Reaching the goal fires once.
  if p.coins >= p.goal and not p._won then
    p._won = true
    play_sound("fanfare")
    action("ui_set_text", { name = "CoinCount", text = "ALL COINS!" })
  end
end

function add_coins(n)
  p.coins = p.coins + n
  play_sound("coin")
  refresh_label()
end

function refresh_label()
  action("ui_set_text", { name = "CoinCount", text = p.coins .. " / " .. p.goal })
end
