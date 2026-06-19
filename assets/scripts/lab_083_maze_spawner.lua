-- lab_083_maze_spawner.lua
-- Procedural maze via recursive-backtracker carving, materialized as walls.
-- WHY: the algorithm needs a grid of cell state the engine has no concept of,
-- so we build it entirely in Lua tables, carve passages, then translate the
-- final wall set into spawn_primitive calls once (spawning during carving
-- would thrash the scene). Setup: attach to a manager; tune W/H/CELL.
-- Multiplayer: layout is RNG-seeded; seed-sync via rpc if peers must match.

local W, H, CELL = 8, 8, 2.0
local walls       -- walls[x][y] = {n,e,s,w} present?
local visited

local function idx(x, y) return x .. ":" .. y end

local function carve(x, y)
  visited[idx(x, y)] = true
  local dirs = { {0,-1,"n","s"}, {1,0,"e","w"}, {0,1,"s","n"}, {-1,0,"w","e"} }
  -- Fisher-Yates shuffle for an unbiased maze.
  for i = #dirs, 2, -1 do
    local j = math.random(i)
    dirs[i], dirs[j] = dirs[j], dirs[i]
  end
  for _, d in ipairs(dirs) do
    local nx, ny = x + d[1], y + d[2]
    if nx >= 1 and nx <= W and ny >= 1 and ny <= H and not visited[idx(nx, ny)] then
      walls[idx(x, y)][d[3]] = false
      walls[idx(nx, ny)][d[4]] = false
      carve(nx, ny)
    end
  end
end

function on_ready()
  walls, visited = {}, {}
  for x = 1, W do for y = 1, H do
    walls[idx(x, y)] = { n = true, e = true, s = true, w = true }
  end end
  carve(1, 1)

  local count = 0
  for x = 1, W do for y = 1, H do
    local cx, cz = x * CELL, y * CELL
    local w = walls[idx(x, y)]
    if w.n then count = count + 1
      spawn_primitive("maze_" .. count, "cube", cx, 1, cz - CELL/2, 0.6, 0.6, 0.7) end
    if w.w then count = count + 1
      spawn_primitive("maze_" .. count, "cube", cx - CELL/2, 1, cz, 0.6, 0.6, 0.7) end
  end end
  print("[maze] spawned " .. count .. " wall segments")
end
