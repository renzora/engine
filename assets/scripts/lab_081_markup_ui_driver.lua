-- lab_081_markup_ui_driver.lua
-- HUI markup panel driver: spawn a template, react to its buttons in on_ui.
-- WHY: markup widgets emit named events back through on_ui(name,args,entity);
-- the script is the controller that owns state and pushes labels via
-- ui_set_text. This is the canonical "view = markup, logic = Lua" split.
-- Setup: provide a HUI template "menu_panel" with buttons emitting
--   "play","options","quit" and a label named "menu_status".
-- Multiplayer: pure local UI; no networking.

local spawned = false
local clicks = 0

function on_ready()
  action("hui_spawn", { template = "menu_panel" })
  spawned = true
  action("ui_set_text", { name = "menu_status", text = "Ready" })
end

function on_ui(name, args, entity)
  clicks = clicks + 1
  if name == "play" then
    action("ui_set_text", { name = "menu_status", text = "Starting..." })
    load_scene("scenes/level1.scene")
  elseif name == "options" then
    action("ui_toggle", { name = "options_drawer" })
    action("ui_set_text", { name = "menu_status",
      text = "Options toggled (" .. clicks .. " clicks)" })
  elseif name == "quit" then
    action("hui_despawn", { template = "menu_panel" })
    action("quit", {})
  else
    -- Unknown event: echo it so template authors can debug bindings.
    action("ui_set_text", { name = "menu_status",
      text = "event: " .. tostring(name) .. " from " .. tostring(entity) })
  end
end
