-- lab_046_double_jump.lua
-- Allow a second mid-air jump by counting jumps until grounded.
-- WHY: we track a jump counter and reset it when is_colliding reports we're
-- touching ground; capping at max_jumps gives the classic double-jump.
-- Setup: entity needs a dynamic RigidBody + Collider; press Space to jump.

local jumps_used = 0

function props()
  return {
    jump_force = { value = 6.0, hint = "Impulse per jump", tab = "Movement" },
    max_jumps = { value = 2, hint = "Total air jumps", tab = "Movement" },
  }
end

function on_update()
  -- Landing (touching something) refills the jump budget.
  if is_colliding then
    jumps_used = 0
  end

  if is_key_just_pressed("Space") and jumps_used < max_jumps then
    -- Zero vertical velocity first so the second jump always feels full-height.
    set_velocity(0.0, jump_force, 0.0)
    jumps_used = jumps_used + 1
  end
end
