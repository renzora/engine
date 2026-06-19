-- lab_053_locomotion_blend.lua
-- WHY: Classic idle/walk/run crossfade locomotion without a state-machine asset.
-- We pick the clip from speed thresholds but only crossfade on a CHANGE, so we
-- don't restart the clip every frame (which would freeze it on frame 0).
-- Setup: character needs "idle", "walk", "run" clips. Tune the thresholds.

function props()
  return {
    walk_threshold = 0.15,
    run_threshold = 0.75,
    fade = 0.25,
    _current = "",
  }
end

function on_ready()
  switch_to("idle")
end

function on_update()
  local mag = math.sqrt(input_x * input_x + input_y * input_y)
  if mag > 1.0 then mag = 1.0 end

  local want
  if mag < p.walk_threshold then
    want = "idle"
  elseif mag < p.run_threshold then
    want = "walk"
  else
    want = "run"
  end

  switch_to(want)
  -- Scale playback so foot speed roughly tracks input (cheap foot-sync).
  if want == "walk" then
    set_animation_speed(0.6 + mag * 0.8)
  elseif want == "run" then
    set_animation_speed(0.9 + mag * 0.6)
  end
end

function switch_to(name)
  if name == p._current then return end
  p._current = name
  crossfade_animation(name, p.fade, true)
end
