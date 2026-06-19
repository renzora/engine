-- lab_086_ragdoll_wrangler.lua
-- Mini physics "ragdoll": spawn limb primitives and shove them with impulses.
-- WHY: real joints aren't exposed to Lua, so we fake articulated chaos by
-- spawning independent rigid cubes and periodically applying impulses + forces
-- to self (the torso) while nudging limbs toward the torso, an attractor-style
-- soft constraint. Setup: attach to a physics-enabled body (the torso).
-- Multiplayer: physics is local; replicate via lab_077 if shared state needed.

local LIMBS = { "head", "armL", "armR", "legL", "legR" }
local offs = { head = {0,1.2,0}, armL = {-0.7,0.5,0}, armR = {0.7,0.5,0},
               legL = {-0.3,-1,0}, legR = {0.3,-1,0} }

local function lname(p) return "rag_" .. p end

function on_ready()
  for _, p in ipairs(LIMBS) do
    local o = offs[p]
    spawn_primitive(lname(p), "cube",
      position_x + o[1], position_y + o[2], position_z + o[3], 0.9, 0.5, 0.4)
  end
  start_timer("twitch", 1.5, true)
end

function on_update()
  -- Soft "spring" pulling each limb back toward its rest offset on the torso.
  for _, p in ipairs(LIMBS) do
    local nm, o = lname(p), offs[p]
    local tx = position_x + o[1]
    local cx = get_on(nm, "Transform.translation.x") or tx
    local k = clamp(delta * 6.0, 0, 1)
    set_on(nm, "Transform.translation.x", lerp(cx, tx, k))
  end

  for _, t in ipairs(timers_finished) do
    if t == "twitch" then
      -- Random kick to keep the ragdoll lively.
      apply_impulse((math.random() - 0.5) * 4, math.random() * 3, (math.random() - 0.5) * 4)
      screen_shake(0.15, 0.2)
    end
  end

  if is_colliding then
    apply_force(0, 8, 0)  -- bounce-ish response when grounded/hit
  end
end
