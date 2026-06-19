-- lab_032_jump_impulse.lua
-- Jump by applying an upward impulse when Space is pressed.
-- WHY: apply_impulse adds instantaneous velocity to a rigid body, which is the
-- correct physics primitive for a jump (a force would ramp up too slowly).
-- Setup: entity needs a dynamic RigidBody + Collider.

function props()
  return {
    jump_force = { value = 6.0, hint = "Upward impulse", tab = "Movement" },
  }
end

function on_update()
  if is_key_just_pressed("Space") then
    apply_impulse(0.0, jump_force, 0.0)
  end
end
