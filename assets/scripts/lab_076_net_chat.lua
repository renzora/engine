-- lab_076_net_chat.lua
-- Multiplayer text-chat bus built on rpc()/on_rpc().
-- WHY: rpc() is the only sanctioned wire primitive, so a "chat" is just a
-- broadcast of {msg, name} that every peer mirrors into a rolling log. The
-- server relays (from==0) but we still trust the embedded name for display.
-- Setup: attach to any always-loaded entity (a manager). Connect first via
--   action("net_connect",{address="127.0.0.1",port=7636}).
-- Multiplayer: each client calls say(); on_rpc rebuilds the shared backlog.

local HISTORY = 8
local log = {}
local pending = {}
local greeted = false

local function push(line)
  log[#log + 1] = line
  while #log > HISTORY do table.remove(log, 1) end
  -- Surface the most recent line into a HUI label if one exists.
  action("ui_set_text", { name = "chat_line", text = line })
end

-- Public helper other scripts can mimic: queue a line to send next frame.
local function say(text)
  pending[#pending + 1] = text
end

function on_ready()
  push("[chat] ready as " .. tostring(self_entity_name))
end

function on_update()
  if not net_is_connected() then return end
  if not greeted then
    greeted = true
    say("joined the channel")
  end
  -- Drain queued messages once per frame to avoid spamming the reliable lane.
  for i = 1, #pending do
    rpc("chat", { name = self_entity_name, msg = pending[i] })
  end
  for i = #pending, 1, -1 do pending[i] = nil end
end

function on_rpc(name, args, from)
  if name ~= "chat" then return end
  local who = args.name or ("peer#" .. tostring(from))
  push(string.format("%s: %s", who, tostring(args.msg)))
end

function on_player_left(id)
  push(string.format("[chat] peer#%d left", id))
end
