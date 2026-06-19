-- lab_099_elevator_network.lua
-- State-replicated elevator: a call/travel/door FSM whose state is mirrored to
-- all peers so everyone rides the same car.
-- WHY: an elevator's position must be identical for all players, so the server
-- runs the FSM (idle->moving->doors) and broadcasts the target floor + phase;
-- clients only interpolate the car toward the authoritative height. Setup:
-- attach to the elevator platform entity; FLOORS lists Y heights.
-- Multiplayer: server owns logic; clients render. Calls come in via rpc.

local FLOORS = { 0.0, 5.0, 10.0, 15.0 }
local cur = 1
local target = 1
local phase = "idle"   -- idle | moving | doors
local car_y = 0.0

local function broadcast()
  rpc("elev", { target = target, phase = phase, y = car_y })
end

function on_ready()
  car_y = FLOORS[cur]
  set_position(position_x, car_y, position_z)
end

function on_update()
  if net_is_client() and not net_is_server() then
    -- Client: smoothly track the last authoritative height.
    local cy = get("Transform.translation.y") or car_y
    set_position(position_x, lerp(cy, car_y, clamp(delta * 3, 0, 1)), position_z)
    return
  end

  -- Server logic below.
  if phase == "moving" then
    local goal = FLOORS[target]
    car_y = lerp(car_y, goal, clamp(delta * 1.5, 0, 1))
    set_position(position_x, car_y, position_z)
    if math.abs(car_y - goal) < 0.05 then
      car_y = goal
      cur = target
      phase = "doors"
      start_timer("doors", 3.0, false)
      play_sound("sfx/elev_ding.ogg")
    end
    broadcast()
  end

  for _, t in ipairs(timers_finished) do
    if t == "doors" then phase = "idle"; broadcast() end
  end

  -- Local call buttons for testing on the host.
  if input_call_up and phase == "idle" and cur < #FLOORS then
    target = cur + 1; phase = "moving"; broadcast()
  elseif input_call_down and phase == "idle" and cur > 1 then
    target = cur - 1; phase = "moving"; broadcast()
  end
end

function on_rpc(name, args, from)
  if name == "elev_call" and net_is_server() and phase == "idle" then
    -- A client requested a floor; validate range then start travel.
    local f = math.floor(args.floor or cur)
    if f >= 1 and f <= #FLOORS and f ~= cur then
      target = f; phase = "moving"; broadcast()
    end
  elseif name == "elev" and net_is_client() then
    target, phase, car_y = args.target, args.phase, args.y
  end
end
