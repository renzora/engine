-- Network Connect
--
-- Attach this to ONE entity in your scene (e.g. an empty named "Net"). On play
-- it connects this client to the dedicated server. Run the server first:
--     renzora-runtime --server
--
-- This script does nothing but connect — keep connection separate from
-- gameplay. Your other networking scripts (multiplayer_ping, net_chat, …)
-- assume the connection already exists. Attach exactly ONE connector; two
-- scripts both calling net_connect will try to open two connections.

function props()
    return {
        address = { type = "string", value = "127.0.0.1", hint = "Server address" },
        port    = { type = "float",  value = 7636,        hint = "Server port (matches --port / project.toml)" },
    }
end

function on_ready()
    action("net_connect", { address = address, port = port })
    print_log("connecting to " .. address .. ":" .. tostring(port))
end
