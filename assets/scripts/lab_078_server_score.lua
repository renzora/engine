-- lab_078_server_score.lua
-- Server-authoritative scoreboard with join/leave lifecycle.
-- WHY: trusting clients with score invites cheating, so only net_is_server()
-- mutates the table; clients send "frag" requests and the server validates,
-- increments, then broadcasts the canonical board back. on_player_joined/left
-- keep the roster honest even across reconnects.
-- Setup: run the same script on host (--server) and clients.
-- Multiplayer: clients call request_point(); only the server's tally counts.

local scores = {}     -- peer id -> points
local dirty = false

local function ensure(id) if not scores[id] then scores[id] = 0 end end

local function broadcast_board()
  -- Flatten to parallel arrays so it survives the rpc table round-trip.
  local ids, pts = {}, {}
  for id, p in pairs(scores) do ids[#ids + 1] = id; pts[#pts + 1] = p end
  rpc("board", { ids = ids, pts = pts })
end

function on_player_joined(id)
  if net_is_server() then ensure(id); dirty = true end
end

function on_player_left(id)
  if net_is_server() then scores[id] = nil; dirty = true end
end

function on_update()
  if net_is_server() and dirty then
    dirty = false
    broadcast_board()
  end
end

function on_rpc(name, args, from)
  if name == "frag" and net_is_server() then
    -- Server is the only place a point is ever minted.
    local target = args.victim or from
    ensure(from)
    scores[from] = scores[from] + 1
    dirty = true
  elseif name == "board" and net_is_client() then
    -- Render the authoritative snapshot locally.
    local top, topid = -1, -1
    for i = 1, #args.ids do
      if args.pts[i] > top then top = args.pts[i]; topid = args.ids[i] end
    end
    action("ui_set_text", { name = "score_leader",
      text = string.format("Leader peer#%d : %d", topid, top) })
  end
end
