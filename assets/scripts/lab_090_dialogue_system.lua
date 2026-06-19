-- lab_090_dialogue_system.lua
-- Branching dialogue engine: fetch a JSON tree, drive a HUI panel, advance on
-- choice events from on_ui.
-- WHY: dialogue is a graph of nodes with choices; we load it once via http_get
-- (json_parse), then treat the current node id as the only mutable state.
-- on_ui("choice",{next=...}) walks the graph and re-renders the panel.
-- Setup: HUI template "dialogue" with a "line" label + choice buttons.
-- Multiplayer: local presentation; choices could be voted via rpc (see lab_097).

local URL = "https://example.com/dialogue/intro.json"
local tree = nil
local node = nil

local function render()
  if not tree or not node then return end
  local n = tree[node]
  if not n then return end
  action("ui_set_text", { name = "dlg_line", text = n.text or "..." })
  -- Label up to three choice buttons; blank the rest.
  for i = 1, 3 do
    local c = n.choices and n.choices[i]
    action("ui_set_text", { name = "dlg_choice" .. i,
      text = c and c.label or "" })
  end
end

function on_ready()
  action("hui_spawn", { template = "dialogue" })
  http_get(URL, "dialogue")
end

function on_http(callback, status, body)
  if callback ~= "dialogue" then return end
  if status == 0 then
    action("ui_set_text", { name = "dlg_line", text = "(failed to load)" })
    return
  end
  local data = json_parse(body)
  if not data then return end
  tree = data.nodes or data
  node = data.start or "start"
  render()
end

function on_ui(name, args, entity)
  if name == "choice" then
    -- Each choice button passes the id of the node to jump to.
    local nxt = args.next
    if nxt == "end" then
      action("hui_despawn", { template = "dialogue" })
      node = nil
    elseif nxt then
      node = nxt
      render()
    end
  end
end
