-- Networked Ping — rpc() / on_rpc() demo
--
-- Press the ping key to broadcast an RPC to every OTHER connected peer. When a
-- ping arrives, `on_rpc` fires on each remote peer (never on the sender).
--
-- Requires a connection — attach `net_connect.lua` to one entity as well, and
-- run the server first (`renzora-runtime --server`). Watch the consoles:
-- pressing the key logs `sent ping` here and `got ping` on the other clients.

function props()
    return {
        ping_key = { type = "string", value = "KeyP",  hint = "Key that broadcasts a ping" },
        message  = { type = "string", value = "hello", hint = "Text sent with each ping" },
    }
end

function on_update()
    if is_key_just_pressed(ping_key) then
        -- Args are an arbitrary table; numbers, strings, bools and {x,y,z}
        -- vectors round-trip. No-op (with a warning) if not connected.
        rpc("ping", { msg = message, at = elapsed })
        print_log("sent ping: " .. tostring(message))
    end
end

-- Fires on remote peers when an RPC arrives. `from` is the sender's peer id
-- (0 = server). A script with no `on_rpc` simply ignores RPCs.
function on_rpc(name, args, from)
    if name == "ping" then
        print_log("got ping from " .. tostring(from) .. ": " .. tostring(args.msg))
    end
end
