-- lab_065_elevator_sequence.lua
-- WHY: A multi-stop elevator. It dwells at each floor for a hold time (timer),
-- then lerps Y to the next stop. Modeling it as an explicit phase ("moving" vs
-- "waiting") plus a stop index keeps the motion deterministic and loopable.
-- Setup: attach to the platform; list floor Y heights in props.stops.

function props()
  return {
    stops = { 0.0, 4.0, 8.0 },
    speed = 2.0,
    hold = 2.5,
    _index = 1,
    _next = 2,
    _phase = "waiting",   -- waiting | moving
  }
end

function on_ready()
  start_timer("dwell", p.hold, false)
end

function on_update()
  if p._phase == "waiting" then
    for _, n in ipairs(timers_finished) do
      if n == "dwell" then begin_move() end
    end
  elseif p._phase == "moving" then
    local target = p.stops[p._next]
    local step = p.speed * delta
    local y = position_y
    if math.abs(target - y) <= step then
      set_position(position_x, target, position_z)
      arrive()
    else
      local dir = (target > y) and 1.0 or -1.0
      set_position(position_x, y + dir * step, position_z)
    end
  end
end

function begin_move()
  p._phase = "moving"
  play_sound("elevator_start")
end

function arrive()
  p._phase = "waiting"
  p._index = p._next
  -- advance to the next stop, wrapping back to the bottom
  p._next = p._next + 1
  if p._next > #p.stops then p._next = 1 end
  play_sound("elevator_stop")
  start_timer("dwell", p.hold, false)
end
