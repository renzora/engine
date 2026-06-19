-- lab_097_net_emote_vote.lua
-- Networked emotes + live voting tally over rpc().
-- WHY: emotes and votes are both tiny broadcast messages; we reuse one rpc
-- surface with a "kind" discriminator. Votes accumulate into a per-option tally
-- that every peer recomputes identically, and an emote spawns a transient
-- billboard primitive above the sender. Setup: attach to each player pawn.
-- Multiplayer: connect first; from==sender id dedupes one vote per peer.

local EMOTES = { "wave", "laugh", "cheer", "cry" }
local tally = {}            -- option -> count
local voted = {}            -- peer id -> option (one vote each)
local emote_n = 0

local function recount()
  tally = {}
  for _, opt in pairs(voted) do tally[opt] = (tally[opt] or 0) + 1 end
  -- Surface the current leader.
  local best, who = -1, "-"
  for opt, c in pairs(tally) do if c > best then best = c; who = opt end end
  action("ui_set_text", { name = "vote_leader",
    text = string.format("Vote leader: %s (%d)", who, math.max(best, 0)) })
end

function on_ready()
  recount()
end

function on_update()
  -- Fire an emote on a key edge (cycles through the list).
  if input_emote then
    local e = EMOTES[math.random(#EMOTES)]
    if net_is_connected() then rpc("net_msg", { kind = "emote", emote = e }) end
  end
  if input_vote_a and net_is_connected() then
    rpc("net_msg", { kind = "vote", opt = "A" })
  end
  if input_vote_b and net_is_connected() then
    rpc("net_msg", { kind = "vote", opt = "B" })
  end
end

function on_rpc(name, args, from)
  if name ~= "net_msg" then return end
  if args.kind == "emote" then
    emote_n = emote_n + 1
    local nm = "emote_" .. from .. "_" .. emote_n
    spawn_primitive(nm, "sphere", position_x, position_y + 2.5, position_z, 1, 1, 0.3)
    print(string.format("peer#%d emotes %s", from, tostring(args.emote)))
  elseif args.kind == "vote" then
    voted[from] = args.opt   -- last vote wins, one slot per peer
    recount()
  end
end

function on_player_left(id)
  if voted[id] then voted[id] = nil; recount() end
end
