-- lab_052_anim_state_driver.lua
-- WHY: Decouples the animation graph from gameplay code. Instead of calling
-- play_animation directly, we feed the animator blend params (Speed float,
-- Grounded/Moving bools, Jump trigger) and let the state machine pick clips.
-- Setup: attach to a character that has an AnimationStateMachine with those params.

function props()
  return {
    move_speed = 4.0,
    _prev_grounded = true,
    _smoothed_speed = 0.0,
  }
end

function on_ready()
  set_anim_bool("Grounded", true)
  set_anim_param("Speed", 0.0)
end

function on_update()
  -- Planar input magnitude (0..1) becomes the locomotion blend Speed.
  local mag = math.sqrt(input_x * input_x + input_y * input_y)
  if mag > 1.0 then mag = 1.0 end
  local target = mag * p.move_speed

  -- Smooth so the blend tree doesn't pop on twitchy input.
  p._smoothed_speed = lerp(p._smoothed_speed, target, clamp(delta * 10.0, 0.0, 1.0))
  set_anim_param("Speed", p._smoothed_speed)
  set_anim_bool("Moving", mag > 0.05)

  local grounded = get("PhysicsReadState.grounded")
  if grounded == nil then grounded = true end
  set_anim_bool("Grounded", grounded)

  -- Rising edge of a jump press fires a one-shot trigger.
  if grounded and is_key_just_pressed("Space") then
    trigger_anim("Jump")
  end

  -- Landing edge: false -> true grounded transition.
  if grounded and not p._prev_grounded then
    trigger_anim("Land")
  end
  p._prev_grounded = grounded
end
