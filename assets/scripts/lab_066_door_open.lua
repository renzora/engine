-- lab_066_door_open.lua
-- WHY: A swinging door driven by an eased angle, not an animation clip, so it
-- works on any mesh. A normalized 0..1 progress is smoothstep-eased and mapped
-- to the open angle; we play the creak once on the open/close edge only.
-- Setup: attach to a door hinged about its own origin; press E to toggle.

function props()
  return {
    open_angle = 95.0,
    open_time = 0.8,
    closed_yaw = 0.0,
    _t = 0.0,         -- 0 = closed, 1 = open
    _open = false,
  }
end

function on_update()
  -- Toggle on the key edge.
  if is_key_just_pressed("KeyE") then
    p._open = not p._open
    play_sound(p._open and "door_open" or "door_close")
  end

  -- March progress toward the target end state.
  local dir = p._open and 1.0 or -1.0
  local before = p._t
  p._t = clamp(p._t + dir * (delta / p.open_time), 0.0, 1.0)
  if p._t == before then return end  -- fully settled, skip work

  -- Smoothstep ease for a soft start/stop, then map to yaw.
  local e = p._t * p._t * (3.0 - 2.0 * p._t)
  set_rotation(0.0, p.closed_yaw + e * p.open_angle, 0.0)
end
