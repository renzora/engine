-- lab_073_landing_shake.lua
-- WHY: Camera juice on landing. We watch the grounded mirror for a false->true
-- edge and scale the screen_shake by how fast we were falling (tracked from the
-- velocity mirror). Only the edge triggers, so cruising on the ground is calm.
-- Setup: attach to a character with PhysicsReadState (grounded + velocity.y).

function props()
  return {
    min_speed = 3.0,      -- below this, no shake
    max_speed = 18.0,     -- shake saturates here
    max_intensity = 0.6,
    _was_grounded = true,
    _fall_speed = 0.0,
  }
end

function on_update()
  local grounded = get("PhysicsReadState.grounded")
  if grounded == nil then grounded = true end

  -- Track the worst downward speed while airborne.
  local vy = get("PhysicsReadState.velocity.y") or 0.0
  if not grounded and vy < 0.0 then
    p._fall_speed = math.max(p._fall_speed, -vy)
  end

  -- Landing edge: scale shake by the impact speed.
  if grounded and not p._was_grounded then
    if p._fall_speed > p.min_speed then
      local f = clamp((p._fall_speed - p.min_speed) / (p.max_speed - p.min_speed), 0.0, 1.0)
      screen_shake(p.max_intensity * f, 0.25 + 0.2 * f)
      play_sound("land_thud")
      trigger_anim("Land")
    end
    p._fall_speed = 0.0
  end

  p._was_grounded = grounded
end
