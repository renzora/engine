-- lab_100_inventory_economy.lua
-- Inventory + economy: a stackable item bag with a shop, HUI display, and
-- cloud persistence via http_post/http_get.
-- WHY: capstone tying many systems together — an item table is the model, HUI
-- the view (ui_set_text), buy/sell mutate gold with validation, and the whole
-- bag round-trips to a server as hand-built JSON so progress survives restarts.
-- Setup: HUI template "inventory" with labels gold/slot1..slotN.
-- Multiplayer: per-player bag keyed by self_entity_name; not shared.

local URL = "https://example.com/inv/"
local gold = 100
local bag = {}            -- item name -> count
local PRICES = { potion = 25, sword = 80, key = 10 }

local function bag_url() return URL .. self_entity_name end

local function refresh_ui()
  action("ui_set_text", { name = "inv_gold", text = "Gold: " .. gold })
  local i = 0
  for item, qty in pairs(bag) do
    i = i + 1
    action("ui_set_text", { name = "inv_slot" .. i,
      text = string.format("%s x%d", item, qty) })
  end
end

local function add(item, n)
  bag[item] = (bag[item] or 0) + n
  if bag[item] <= 0 then bag[item] = nil end
end

local function buy(item)
  local price = PRICES[item]
  if not price then return false end
  if gold < price then
    action("ui_set_text", { name = "inv_msg", text = "Not enough gold!" })
    return false
  end
  gold = gold - price
  add(item, 1)
  refresh_ui()
  return true
end

local function sell(item)
  if not bag[item] then return false end
  gold = gold + math.floor((PRICES[item] or 0) * 0.5)
  add(item, -1)
  refresh_ui()
  return true
end

local function save()
  -- Encode the bag as JSON manually (no encoder in the API).
  local parts = {}
  for item, qty in pairs(bag) do
    parts[#parts + 1] = string.format('"%s":%d', item, qty)
  end
  local body = string.format('{"gold":%d,"bag":{%s}}', gold, table.concat(parts, ","))
  http_post(bag_url(), body, "inv_save")
end

function on_ready()
  action("hui_spawn", { template = "inventory" })
  http_get(bag_url(), "inv_load")
  start_timer("inv_autosave", 30.0, true)
end

function on_update()
  for _, t in ipairs(timers_finished) do
    if t == "inv_autosave" then save() end
  end
end

function on_ui(name, args, entity)
  if name == "buy" then
    buy(args.item)
  elseif name == "sell" then
    sell(args.item)
  elseif name == "save" then
    save()
  end
end

function on_http(callback, status, body)
  if callback == "inv_load" then
    if status == 0 then refresh_ui(); return end
    local d = json_parse(body)
    if d then
      gold = d.gold or gold
      bag = d.bag or {}
    end
    refresh_ui()
  elseif callback == "inv_save" then
    action("ui_set_text", { name = "inv_msg",
      text = status == 0 and "save failed" or "saved" })
  end
end
