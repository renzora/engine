-- Networked Chat — rpc() with arguments + sender id
--
-- Press 1 / 2 / 3 to broadcast a preset line to every other peer. Each peer's
-- `on_rpc` logs "[player <id>] <text>". Demonstrates sending string args over
-- an RPC and reading who it came from.
--
-- Requires a connection (attach `net_connect.lua`, run the server first).

function props()
    return {
        line1 = { type = "string", value = "hello!",      hint = "Sent on key 1" },
        line2 = { type = "string", value = "gg",          hint = "Sent on key 2" },
        line3 = { type = "string", value = "on my way",   hint = "Sent on key 3" },
    }
end

local function say(text)
    rpc("chat", { text = text })   -- broadcast to other peers
    print_log("you: " .. text)     -- local echo (rpc() never echoes to sender)
end

function on_update()
    if is_key_just_pressed("Digit1") then say(line1) end
    if is_key_just_pressed("Digit2") then say(line2) end
    if is_key_just_pressed("Digit3") then say(line3) end
end

function on_rpc(name, args, from)
    if name == "chat" then
        print_log("[player " .. tostring(from) .. "] " .. tostring(args.text))
    end
end
