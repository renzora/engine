-- lab_092_hot_inspector.lua
-- Reflection "hot inspector": once a second, dump every component on self and
-- watch chosen fields for changes.
-- WHY: get_components() + get_component() expose the live reflected state, so a
-- script can act as a debugger overlay without engine support. We diff watched
-- paths frame-to-frame and only log on change to avoid console spam (a known
-- frame-stall trap). Setup: attach to any entity you want to introspect.
-- Multiplayer: read-only; safe anywhere.

local WATCH = { "Transform.translation.y", "Transform.scale.x" }
local last = {}

local function snapshot()
  local comps = get_components()
  if not comps then return end
  local n = 0
  for _ in pairs(comps) do n = n + 1 end
  print(string.format("[inspect] %s has %d components", self_entity_name, n))
  for tname, _ in pairs(comps) do
    local data = get_component(tname)
    if type(data) == "table" then
      -- Print only top-level keys to keep the dump readable.
      local keys = {}
      for k in pairs(data) do keys[#keys + 1] = k end
      print("  - " .. tname .. " { " .. table.concat(keys, ", ") .. " }")
    end
  end
end

function on_ready()
  start_timer("inspect", 1.0, true)
  for _, p in ipairs(WATCH) do last[p] = get(p) end
end

function on_update()
  -- Per-frame change detection on watched paths.
  for _, p in ipairs(WATCH) do
    local v = get(p)
    if v ~= last[p] then
      print(string.format("[watch] %s: %s -> %s", p, tostring(last[p]), tostring(v)))
      last[p] = v
    end
  end

  for _, t in ipairs(timers_finished) do
    if t == "inspect" then snapshot() end
  end
end
