-- lab_079_http_leaderboard.lua
-- Async leaderboard fetch: http_get -> on_http -> json_parse -> UI.
-- WHY: HTTP is fire-and-forget; the response only arrives in on_http, so we
-- tag each request with a callback name and key our parsing on it. status==0
-- means the transport failed (body holds the error) and we must degrade
-- gracefully rather than json_parse garbage.
-- Setup: point URL at any JSON endpoint returning {entries:[{name,score}]}.
-- Multiplayer: read-only, safe to run on every client independently.

local URL = "https://example.com/api/leaderboard"
local REFRESH = 30.0
local last_top = "(loading)"

local function request()
  http_get(URL, "leaderboard")
end

function on_ready()
  request()
  start_timer("lb_refresh", REFRESH, true)
end

function on_update()
  for _, t in ipairs(timers_finished) do
    if t == "lb_refresh" then request() end
  end
end

function on_http(callback, status, body)
  if callback ~= "leaderboard" then return end
  if status == 0 then
    last_top = "fetch failed: " .. tostring(body)
    action("ui_set_text", { name = "lb_status", text = last_top })
    return
  end
  local data = json_parse(body)
  if not data or not data.entries then
    action("ui_set_text", { name = "lb_status", text = "bad payload" })
    return
  end
  -- Find the highest score defensively (don't assume server pre-sorts).
  local best, name = -1, "?"
  for i = 1, #data.entries do
    local e = data.entries[i]
    if e.score and e.score > best then best = e.score; name = e.name or "?" end
  end
  last_top = string.format("#1 %s (%d)", name, best)
  action("ui_set_text", { name = "lb_status", text = last_top })
end
