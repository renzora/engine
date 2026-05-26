-- Server-authoritative score — net_is_server() + rpc round-trip
--
-- Any client presses the key to request a point. Only the SERVER owns the
-- count (gated by net_is_server()); it increments and broadcasts the new total
-- to everyone. This is the authoritative pattern — clients ask, the server
-- decides — built entirely from working primitives, nothing hardcoded.
--
-- Requires a connection (attach net_connect.lua, run the server first). Watch
-- the server console for the tally and every client for "SCORE: N".

function props()
    return {
        add_key = { type = "string", value = "KeyK", hint = "Press to request a point" },
        _score  = { type = "float",  value = 0,      hint = "Internal: server-side total" },
    }
end

function on_update()
    if is_key_just_pressed(add_key) then
        rpc("score_request", {})   -- ask the server (broadcast; only it acts)
    end
end

function on_rpc(name, args, from)
    if name == "score_request" then
        -- Only the server tallies. Clients receive this too but skip it.
        if net_is_server() then
            _score = _score + 1
            rpc("score_update", { total = _score })   -- tell everyone
            print_log("player " .. tostring(from) .. " scored — total " .. tostring(_score))
        end
    elseif name == "score_update" then
        print_log("SCORE: " .. tostring(args.total))
    end
end
