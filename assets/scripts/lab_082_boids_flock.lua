-- lab_082_boids_flock.lua
-- Reynolds boids: spawn N primitives and steer them with cohesion, alignment
-- and separation each frame.
-- WHY: real flocking needs per-agent state the engine doesn't track for us, so
-- we keep a Lua table of {pos,vel} and write back via set_on() to the spawned
-- entities. O(n^2) neighbour scan is fine for small N and keeps it dependency
-- free. Setup: attach to an empty manager; adjust COUNT.
-- Multiplayer: visual only; run on each client (deterministic-ish, not synced).

local COUNT = 24
local RADIUS = 4.0
local MAXSPD = 3.0
local boids = {}

local function name(i) return "boid_" .. i end

local function clamp_speed(v)
  local m = math.sqrt(v.x * v.x + v.y * v.y + v.z * v.z)
  if m > MAXSPD and m > 0 then
    local s = MAXSPD / m
    v.x, v.y, v.z = v.x * s, v.y * s, v.z * s
  end
end

function on_ready()
  for i = 1, COUNT do
    local x = math.random() * 10 - 5
    local z = math.random() * 10 - 5
    spawn_primitive(name(i), "cube", x, 2.0, z, 0.3, 0.8, 0.5)
    boids[i] = { pos = vec3(x, 2.0, z),
                 vel = vec3(math.random() - 0.5, 0, math.random() - 0.5) }
  end
end

function on_update()
  local dt = clamp(delta, 0, 0.05)
  for i = 1, COUNT do
    local b = boids[i]
    local coh, ali, sep = vec3(0,0,0), vec3(0,0,0), vec3(0,0,0)
    local n = 0
    for j = 1, COUNT do
      if i ~= j then
        local o = boids[j]
        local dx, dy, dz = o.pos.x - b.pos.x, o.pos.y - b.pos.y, o.pos.z - b.pos.z
        local d = math.sqrt(dx*dx + dy*dy + dz*dz)
        if d < RADIUS and d > 0 then
          coh.x, coh.z = coh.x + o.pos.x, coh.z + o.pos.z
          ali.x, ali.z = ali.x + o.vel.x, ali.z + o.vel.z
          sep.x, sep.z = sep.x - dx / d, sep.z - dz / d
          n = n + 1
        end
      end
    end
    if n > 0 then
      coh.x = (coh.x / n - b.pos.x) * 0.5
      coh.z = (coh.z / n - b.pos.z) * 0.5
      ali.x, ali.z = ali.x / n * 0.6, ali.z / n * 0.6
      sep.x, sep.z = sep.x * 1.4, sep.z * 1.4
      b.vel.x = b.vel.x + (coh.x + ali.x + sep.x) * dt
      b.vel.z = b.vel.z + (coh.z + ali.z + sep.z) * dt
    end
    clamp_speed(b.vel)
    b.pos.x = b.pos.x + b.vel.x * dt
    b.pos.z = b.pos.z + b.vel.z * dt
    set_on(name(i), "Transform.translation.x", b.pos.x)
    set_on(name(i), "Transform.translation.z", b.pos.z)
  end
end
