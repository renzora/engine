-- lab_055_platformer_coyote.lua
-- WHY: Forgiving platformer jump. Coyote-time lets you jump a few frames after
-- leaving a ledge; jump-buffer lets a press land slightly before touchdown still
-- count. Both hide input latency and feel far better than a strict grounded check.
-- Setup: attach to a physics character with PhysicsReadState.grounded.

function props()
  return {
    move_speed = 6.0,
    jump_force = 9.0,
    gravity = -22.0,
    coyote = 0.12,      -- seconds of grace after leaving ground
    buffer = 0.12,      -- seconds a jump press stays queued
    _vy = 0.0,
    _coyote_t = 0.0,
    _buffer_t = 0.0,
  }
end

function on_update()
  local grounded = get("PhysicsReadState.grounded") or false

  -- Refresh / decay the two timers.
  if grounded then p._coyote_t = p.coyote else p._coyote_t = p._coyote_t - delta end
  if is_key_just_pressed("Space") then p._buffer_t = p.buffer else p._buffer_t = p._buffer_t - delta end

  -- A jump is allowed when a buffered press meets available coyote time.
  if p._buffer_t > 0.0 and p._coyote_t > 0.0 then
    p._vy = p.jump_force
    p._buffer_t = 0.0
    p._coyote_t = 0.0
    play_sound("jump")
    trigger_anim("Jump")
  end

  -- Gravity + a tiny grounded clamp.
  if grounded and p._vy <= 0.0 then p._vy = -1.0 else p._vy = p._vy + p.gravity * delta end

  move_controller(input_x * p.move_speed * delta, p._vy * delta, input_y * p.move_speed * delta)
end
