-- Net HUD — live connection dashboard.
--
-- Shows this peer's role + connection state on screen every frame. Attach to
-- ONE entity. Create a UI Canvas named "HUD" with a Text widget named
-- "NetStatus" (the `ui_set_text` calls find widgets by Name; if it doesn't
-- exist they're harmless no-ops, so this also works console-only).
--
-- Pairs with net_connect.lua (to connect) and the rest of the test kit.

local function ui(name, text)
    action("ui_set_text", { name = name, text = text })
end

function on_update()
    local role = net_is_server() and "SERVER" or "CLIENT"
    local conn = net_is_connected() and "connected" or "offline"
    ui("NetStatus", role .. " — " .. conn)

    -- Player count is authoritative on the server/host. Clients get it from
    -- net_lobby's broadcast instead.
    if net_is_server() then
        ui("Players", "Players: " .. tostring(net_player_count()))
    end
end
