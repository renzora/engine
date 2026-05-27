-- Net Lobby — presence tracking via the server-side player-lifecycle hooks.
--
-- The SERVER observes connects/disconnects through `on_player_joined(id)` /
-- `on_player_left(id)` (these fire only on the server/host), keeps an
-- authoritative player count, and broadcasts it so every client's HUD updates
-- too. Demonstrates the "who's in the game" primitive — nothing hardcoded.
--
-- HUD: create Text widgets named "Players" and "Events" under a "HUD" canvas.
-- Attach this to ONE entity. Requires net_connect.lua + a running server.

local count = 0

local function set_players(n)
    action("ui_set_text", { name = "Players", text = "Players: " .. tostring(n) })
end
local function set_event(s)
    action("ui_set_text", { name = "Events", text = s })
end

-- ── Server side: lifecycle hooks fire only here ────────────────────────────
function on_player_joined(id)
    count = count + 1
    print_log("[server] player " .. tostring(id) .. " joined — " .. tostring(count) .. " online")
    set_players(count)
    set_event("player " .. tostring(id) .. " joined")
    rpc("lobby", { count = count, who = id, joined = true })   -- tell the clients
end

function on_player_left(id)
    if count > 0 then count = count - 1 end
    print_log("[server] player " .. tostring(id) .. " left — " .. tostring(count) .. " online")
    set_players(count)
    set_event("player " .. tostring(id) .. " left")
    rpc("lobby", { count = count, who = id, joined = false })
end

-- ── Client side: receive the broadcast presence update ─────────────────────
function on_rpc(name, args, from)
    if name == "lobby" then
        set_players(args.count)
        set_event("player " .. tostring(args.who) .. (args.joined and " joined" or " left"))
    end
end
