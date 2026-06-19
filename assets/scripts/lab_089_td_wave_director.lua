-- lab_089_td_wave_director.lua
-- Tower-defense wave director: a state machine spawning escalating enemy waves.
-- WHY: pacing requires a timer-driven FSM (intermission -> spawning -> clear)
-- the engine won't orchestrate for us. We track alive count by spawn/despawn
-- bookkeeping and only advance the wave when the field is clear, scaling enemy
-- count and speed each round. Setup: attach to a manager near a path origin.
-- Multiplayer: server-authoritative spawning recommended (gate on net_is_server).

local state = "intermission"
local wave = 0
local to_spawn = 0
local alive = 0
local spawn_id = 0

local function begin_intermission()
  state = "intermission"
  action("ui_set_text", { name = "td_status", text = "Wave " .. wave .. " cleared!" })
  start_timer("td_break", 5.0, false)
end

local function start_wave()
  wave = wave + 1
  to_spawn = 4 + wave * 2
  state = "spawning"
  action("ui_set_text", { name = "td_status", text = "Wave " .. wave .. " incoming" })
  start_timer("td_spawn", math.max(0.3, 1.2 - wave * 0.05), true)
end

function on_ready()
  if net_is_client() and not net_is_server() then return end
  start_wave()
end

function on_update()
  for _, t in ipairs(timers_finished) do
    if t == "td_break" then
      start_wave()
    elseif t == "td_spawn" and state == "spawning" then
      if to_spawn > 0 then
        spawn_id = spawn_id + 1
        to_spawn = to_spawn - 1
        alive = alive + 1
        local hp = 10 + wave * 5
        spawn_primitive("enemy_" .. spawn_id, "capsule",
          math.random() * 4 - 2, 1, -10, 0.9, 0.3, 0.3)
      else
        stop_timer("td_spawn")
        state = "clearing"
      end
    end
  end

  -- When clearing and nothing remains, loop back to intermission.
  if state == "clearing" and alive <= 0 then
    begin_intermission()
  end
end

-- Other scripts (an enemy's own script) report a kill via rpc/action; here we
-- expose a decrement through on_rpc so the director stays authoritative.
function on_rpc(name, args, from)
  if name == "enemy_down" then alive = math.max(0, alive - 1) end
end
