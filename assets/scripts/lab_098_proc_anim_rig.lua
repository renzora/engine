-- lab_098_proc_anim_rig.lua
-- Procedural animation blender: cross-fade locomotion layers by driving
-- set_layer_weight from movement speed and state.
-- WHY: a responsive character blends idle/walk/run continuously rather than
-- snapping clips; we compute target weights from input magnitude, smooth them
-- (avoids popping), and also fire an upper-body action layer on demand. Setup:
-- attach to a rigged character with layers 0..2 + an "action" layer.
-- Multiplayer: drive remotely by replicating speed via rpc if needed.

local LOCO = { idle = 0, walk = 1, run = 2 }
local w = { 1.0, 0.0, 0.0 }   -- current smoothed weights
local action_w = 0.0

local function smooth(cur, target, rate)
  return cur + (target - cur) * clamp(delta * rate, 0, 1)
end

function on_ready()
  play_animation("idle")
  set_layer_weight(LOCO.idle, 1.0)
end

function on_update()
  -- Derive a speed scalar from movement input.
  local mx = input_move_x or 0
  local mz = input_move_z or 0
  local speed = clamp(math.sqrt(mx * mx + mz * mz), 0, 1)
  local running = input_sprint and speed > 0.1

  -- Target weights: idle<->walk<->run as a 1D blend space.
  local t_idle = 1.0 - speed
  local t_walk = running and 0.0 or speed
  local t_run  = running and speed or 0.0

  w[1] = smooth(w[1], t_idle, 8)
  w[2] = smooth(w[2], t_walk, 8)
  w[3] = smooth(w[3], t_run, 8)
  set_layer_weight(LOCO.idle, w[1])
  set_layer_weight(LOCO.walk, w[2])
  set_layer_weight(LOCO.run, w[3])
  set_anim_param("speed", speed)

  -- Upper-body action overlay (e.g. wave) fades in on a key, then decays.
  if input_action then
    action_w = 1.0
    set_anim_trigger("wave")
  end
  action_w = smooth(action_w, 0.0, 3)
  set_layer_weight(3, action_w)
end
