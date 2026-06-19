-- lab_069_layered_anim_blend.lua
-- WHY: Additive upper-body layering. A base locomotion clip plays on layer 0
-- while an "aim"/"wave" overlay rides layer 1. We ramp the layer-1 weight in/out
-- so the overlay fades smoothly instead of snapping the upper body in place.
-- Setup: character has a layered animator; layer 1 holds the overlay clip.

function props()
  return {
    fade_speed = 4.0,
    _weight = 0.0,
    _target = 0.0,
    _overlay_on = false,
  }
end

function on_ready()
  play_animation("idle", true)            -- base layer
  set_layer_weight(1, 0.0)                -- overlay starts silent
end

function on_update()
  -- Hold the right mouse / R2 to engage the upper-body overlay (e.g. aim).
  local engage = mouse_right or gamepad_r2 > 0.5
  if engage ~= p._overlay_on then
    p._overlay_on = engage
    p._target = engage and 1.0 or 0.0
    if engage then crossfade_animation("aim_overlay", 0.1, true) end
  end

  -- Ramp the weight toward its target every frame.
  if p._weight ~= p._target then
    p._weight = lerp(p._weight, p._target, clamp(delta * p.fade_speed, 0.0, 1.0))
    if math.abs(p._weight - p._target) < 0.01 then p._weight = p._target end
    set_layer_weight(1, p._weight)
  end
end
