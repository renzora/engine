-- lab_093_http_save_load.lua
-- Cloud save/load: serialize reflected component state to a server via
-- http_post, and restore it via http_get + json_parse on demand.
-- WHY: persistence needs a snapshot of live component data; get_component()
-- gives us the table, we hand-encode it to JSON (no encoder available), POST
-- it, and on load we set() each field back. status==0 handling avoids
-- corrupting state on a failed fetch. Setup: attach to a saveable entity.
-- Multiplayer: per-entity save; keyed by self_entity_name on the server.

local BASE = "https://example.com/saves/"

local function url() return BASE .. self_entity_name end

local function save()
  local t = get_component("Transform")
  if not t or not t.translation then return end
  -- Hand-build a compact JSON document of the pose.
  local body = string.format(
    '{"x":%.3f,"y":%.3f,"z":%.3f,"rx":%.2f,"ry":%.2f,"rz":%.2f}',
    position_x, position_y, position_z, rotation_x, rotation_y, rotation_z)
  http_post(url(), body, "save")
end

local function load()
  http_get(url(), "load")
end

function on_ready()
  load()
  start_timer("autosave", 30.0, true)
end

function on_update()
  for _, t in ipairs(timers_finished) do
    if t == "autosave" then save() end
  end
  -- Manual save bound to a key edge.
  if input_quicksave then save() end
end

function on_http(callback, status, body)
  if status == 0 then
    print("[save] " .. callback .. " failed: " .. tostring(body))
    return
  end
  if callback == "load" then
    local d = json_parse(body)
    if not d then return end
    set_position(d.x or position_x, d.y or position_y, d.z or position_z)
    set_rotation(d.rx or 0, d.ry or 0, d.rz or 0)
    print("[save] restored " .. self_entity_name)
  elseif callback == "save" then
    print("[save] persisted " .. self_entity_name)
  end
end
