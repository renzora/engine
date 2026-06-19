-- lab_095_attractor_swarm.lua
-- Swarm that conforms to a moving attractor point orbiting the world.
-- WHY: this is a "conform to sphere/attractor" behaviour done in script: the
-- attractor traces a Lissajous path each frame and every swarm member is pulled
-- toward it with critically-damped easing, so the cloud breathes and follows
-- without overshoot. Setup: attach to a manager; SWARM sets particle count.
-- Multiplayer: deterministic from elapsed, so peers see the same swarm.

local SWARM = 40
local agents = {}

local function aname(i) return "swarm_" .. i end

function on_ready()
  for i = 1, SWARM do
    local x = math.random() * 8 - 4
    local y = math.random() * 8 - 4
    local z = math.random() * 8 - 4
    spawn_primitive(aname(i), "sphere", x, y + 3, z, 0.9, 0.5, 1.0)
    set_scale_uniform(0.25)
    agents[i] = vec3(x, y + 3, z)
  end
end

function on_update()
  -- Moving attractor: a 3D Lissajous figure for organic motion.
  local t = elapsed
  local ax = math.sin(t * 0.7) * 5
  local ay = 3 + math.sin(t * 1.3) * 2
  local az = math.cos(t * 0.9) * 5
  draw_line(ax, ay, az, ax, ay + 0.5, az, 0.1)

  local k = clamp(delta * 2.5, 0, 1)
  for i = 1, SWARM do
    local a = agents[i]
    -- Per-agent jitter so they don't collapse to a single point.
    local jx = math.sin(t + i) * 0.6
    local jz = math.cos(t * 1.1 + i) * 0.6
    a.x = lerp(a.x, ax + jx, k)
    a.y = lerp(a.y, ay + math.sin(i) * 0.6, k)
    a.z = lerp(a.z, az + jz, k)
    local nm = aname(i)
    set_on(nm, "Transform.translation.x", a.x)
    set_on(nm, "Transform.translation.y", a.y)
    set_on(nm, "Transform.translation.z", a.z)
  end
end
