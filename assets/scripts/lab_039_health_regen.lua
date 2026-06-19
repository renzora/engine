-- lab_039_health_regen.lua
-- Regenerate health over time, capped at the maximum.
-- WHY: we track current health in a prop (so it survives across frames and is
-- inspector-visible) and accumulate fractional regen with delta so the rate is
-- exact regardless of frame rate.
-- Setup: tune regen_per_sec and max_health; reads self_health to seed.

function props()
  return {
    health = { value = 100.0, hint = "Current health", tab = "Stats" },
    max_health = { value = 100.0, hint = "Cap", tab = "Stats" },
    regen_per_sec = { value = 5.0, hint = "HP per second", tab = "Stats" },
  }
end

function on_update()
  if health >= max_health then return end
  health = clamp(health + regen_per_sec * delta, 0.0, max_health)
end
