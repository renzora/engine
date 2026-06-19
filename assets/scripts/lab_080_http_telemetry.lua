-- lab_080_http_telemetry.lua
-- Batched telemetry uploader via http_post with a JSON body string.
-- WHY: posting one event per frame would hammer the endpoint, so we buffer
-- samples and flush on an interval, hand-building the JSON string (no encoder
-- in the API). on_http confirms or, on status==0, re-queues the batch so a
-- transient network blip doesn't lose data.
-- Setup: set URL; events accrete from gameplay (here: position deltas).
-- Multiplayer: each client reports independently; server can run it too.

local URL = "https://example.com/api/telemetry"
local FLUSH = 5.0
local buffer = {}
local inflight = nil
local last = nil

local function jstr(s) return '"' .. tostring(s):gsub('"', '\\"') .. '"' end

local function record(kind, value)
  buffer[#buffer + 1] = string.format('{"t":%.2f,"k":%s,"v":%.3f}',
    elapsed, jstr(kind), value)
end

local function flush()
  if #buffer == 0 then return end
  inflight = "[" .. table.concat(buffer, ",") .. "]"
  buffer = {}
  http_post(URL, '{"session":' .. jstr(self_entity_name) ..
    ',"events":' .. inflight .. "}", "telemetry")
end

function on_ready()
  last = vec3(position_x, position_y, position_z)
  start_timer("tele_flush", FLUSH, true)
end

function on_update()
  -- Sample how far we moved since last frame as a trivial metric.
  local dx = position_x - last.x
  local dz = position_z - last.z
  local dist = math.sqrt(dx * dx + dz * dz)
  if dist > 0.01 then record("move", dist) end
  last = vec3(position_x, position_y, position_z)

  for _, t in ipairs(timers_finished) do
    if t == "tele_flush" then flush() end
  end
end

function on_http(callback, status, body)
  if callback ~= "telemetry" then return end
  if status == 0 then
    -- Re-queue the failed batch as a single blob so nothing is dropped.
    if inflight then buffer[#buffer + 1] = inflight end
  end
  inflight = nil
end
