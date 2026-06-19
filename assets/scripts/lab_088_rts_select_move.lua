-- lab_088_rts_select_move.lua
-- RTS unit director: spawn a squad, select via key, issue nav move-orders.
-- WHY: an RTS needs a notion of "selected set" + destination the engine has no
-- single component for, so the controller owns the squad table and uses
-- nav_set_destination per unit (with a formation offset) to spread them around
-- the click target. Setup: attach to a manager on a navmesh scene.
-- Multiplayer: orders could be broadcast via rpc for shared command (TODO).

local SQUAD = 6
local units = {}
local selected = {}

local function uname(i) return "unit_" .. i end

function on_ready()
  for i = 1, SQUAD do
    spawn_primitive(uname(i), "capsule", i * 1.5 - 4, 1, 0, 0.3, 0.7, 1.0)
    units[i] = { name = uname(i) }
    selected[i] = true  -- start with whole squad selected
  end
end

local function formation_offset(i)
  -- Simple 3-wide grid so units don't stack on one point.
  local col = (i - 1) % 3
  local row = math.floor((i - 1) / 3)
  return col * 1.2 - 1.2, row * 1.2
end

function on_update()
  -- Toggle selection of the whole squad with a key edge (input_* booleans).
  if input_select_all then
    for i = 1, SQUAD do selected[i] = true end
  end

  -- Issue a move order on click toward the mouse-world target.
  if mouse_left then
    local tx = mouse_world_x or position_x
    local tz = mouse_world_z or position_z
    for i = 1, SQUAD do
      if selected[i] then
        local ox, oz = formation_offset(i)
        action_on(units[i].name, "noop", {})
        nav_set_destination(tx + ox, 0, tz + oz)
      end
    end
    draw_line(tx - 0.5, 0.1, tz, tx + 0.5, 0.1, tz, 0.5)
  end

  -- Right click clears destinations (hold position).
  if mouse_right then
    for i = 1, SQUAD do nav_clear_destination() end
  end
end
