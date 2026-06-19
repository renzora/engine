-- lab_077_net_position_sync.lua
-- Client-side networked transform replication over rpc().
-- WHY: there is no built-in Transform replication, so we hand-roll it: each
-- owner broadcasts its pose on a fixed tick, and remote peers spawn a "ghost"
-- primitive per id and lerp it toward the last received sample (dead-reckoning
-- lite) to hide jitter between sparse updates.
-- Setup: attach to the locally controlled entity.
-- Multiplayer: connect, then every peer sees a smoothed cube for every owner.

local SEND_HZ = 10
local acc = 0
local ghosts = {}   -- from -> { tx,ty,tz, name }

local function ghost_name(from) return "ghost_" .. tostring(from) end

function on_ready()
  start_timer("net_tick", 1.0 / SEND_HZ, true)
end

function on_update()
  -- Broadcast our own pose on the repeating timer edge.
  for _, t in ipairs(timers_finished) do
    if t == "net_tick" and net_is_connected() then
      rpc("pose", { x = position_x, y = position_y, z = position_z,
                    yaw = rotation_y })
    end
  end

  -- Smooth every known ghost toward its target each frame.
  for from, g in pairs(ghosts) do
    local nm = ghost_name(from)
    local cx = get_on(nm, "Transform.translation.x") or g.tx
    local cy = get_on(nm, "Transform.translation.y") or g.ty
    local cz = get_on(nm, "Transform.translation.z") or g.tz
    local a = clamp(delta * 12.0, 0, 1)
    action_on(nm, "noop", {}) -- keep target alive; harmless verb guard
    set_on(nm, "Transform.translation.x", lerp(cx, g.tx, a))
    set_on(nm, "Transform.translation.y", lerp(cy, g.ty, a))
    set_on(nm, "Transform.translation.z", lerp(cz, g.tz, a))
  end
end

function on_rpc(name, args, from)
  if name ~= "pose" or from == 0 then return end
  local g = ghosts[from]
  if not g then
    spawn_primitive(ghost_name(from), "cube", args.x, args.y, args.z, 0.2, 0.7, 1.0)
    g = {}
    ghosts[from] = g
  end
  g.tx, g.ty, g.tz = args.x, args.y, args.z
end

function on_player_left(id)
  if ghosts[id] then
    despawn_by_prefix(ghost_name(id))
    ghosts[id] = nil
  end
end
