-- lab_085_lsystem_fractal.lua
-- L-system turtle: expand a grammar to a string, then walk it placing cubes.
-- WHY: fractals are defined by string rewriting the engine can't do natively,
-- so we iterate the production rules in Lua and use a turtle stack ([ and ])
-- to branch. Each forward step drops a small primitive, building a plant-like
-- structure. Setup: attach to a manager; tweak ITERS (growth is exponential!).
-- Multiplayer: deterministic; identical on all peers (no RNG).

local AXIOM = "F"
local RULES = { F = "FF+[+F-F-F]-[-F+F+F]" }
local ITERS = 3
local ANGLE = 25.0
local STEP = 0.5

local function expand(s)
  local out = {}
  for c in s:gmatch(".") do out[#out + 1] = RULES[c] or c end
  return table.concat(out)
end

function on_ready()
  local s = AXIOM
  for _ = 1, ITERS do s = expand(s) end

  -- Turtle state in 2D (XY plane), heading in degrees.
  local x, y, heading = 0.0, 0.0, 90.0
  local stack = {}
  local n = 0
  for c in s:gmatch(".") do
    if c == "F" then
      local rad = math.rad(heading)
      local nx = x + math.cos(rad) * STEP
      local ny = y + math.sin(rad) * STEP
      n = n + 1
      spawn_primitive("frac_" .. n, "cube", nx, ny + 1, 0, 0.3, 0.8, 0.3)
      x, y = nx, ny
    elseif c == "+" then heading = heading + ANGLE
    elseif c == "-" then heading = heading - ANGLE
    elseif c == "[" then stack[#stack + 1] = { x, y, heading }
    elseif c == "]" then
      local t = table.remove(stack)
      if t then x, y, heading = t[1], t[2], t[3] end
    end
  end
  print("[lsys] placed " .. n .. " segments from " .. #s .. " symbols")
end
