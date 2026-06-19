-- lab_084_wave_field.lua
-- Animated cube field: a grid of primitives driven by a travelling sine wave.
-- WHY: a single mesh can't show per-cell motion, so we spawn a lattice once
-- and only update the cheap Transform.y + a color ramp each frame. Phase is a
-- function of radial distance from centre, giving a ripple/heightmap feel.
-- Setup: attach to a manager; GRID controls resolution (GRID^2 entities).
-- Multiplayer: deterministic from elapsed; visually consistent across peers.

local GRID = 12
local SPACING = 1.0
local AMP = 1.5

local function name(i, j) return "wf_" .. i .. "_" .. j end

function on_ready()
  for i = 1, GRID do for j = 1, GRID do
    local x = (i - GRID / 2) * SPACING
    local z = (j - GRID / 2) * SPACING
    spawn_primitive(name(i, j), "cube", x, 0, z, 0.2, 0.4, 0.9)
    set_scale_uniform(0.4)  -- note: applies to self; cells scaled at spawn size
  end end
end

function on_update()
  local t = elapsed * 2.0
  for i = 1, GRID do for j = 1, GRID do
    local x = (i - GRID / 2) * SPACING
    local z = (j - GRID / 2) * SPACING
    local r = math.sqrt(x * x + z * z)
    local h = math.sin(r * 0.8 - t) * AMP
    local nm = name(i, j)
    set_on(nm, "Transform.translation.y", h)
    -- Map height to a blue->cyan ramp so crests read clearly.
    local c = clamp((h + AMP) / (2 * AMP), 0, 1)
    set_on(nm, "Transform.scale.y", 0.4 + c * 0.6)
  end end
end
