-- lab_057_grounded_jump_gate.lua
-- WHY: The simplest correct jump: only allow it when the physics mirror says we
-- are actually grounded. Reading PhysicsReadState.grounded (not guessing from Y)
-- avoids double-jumps off slopes and mid-air re-jumps.
-- Setup: attach to a dynamic rigid body that exposes PhysicsReadState.

function props()
  return {
    jump_impulse = 7.0,
    _was_grounded = false,
  }
end

function on_update()
  local grounded = get("PhysicsReadState.grounded")
  if grounded == nil then grounded = false end

  -- Gate: a fresh press AND feet on the ground.
  if grounded and is_key_just_pressed("Space") then
    apply_impulse(0.0, p.jump_impulse, 0.0)
    play_sound("jump")
  end

  -- Edge log: print only on the landing transition, not every frame.
  if grounded and not p._was_grounded then
    print_log("landed")
  end
  p._was_grounded = grounded
end
