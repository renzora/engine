-- Example: gamepad-driven tab browser.
-- Pair with `assets/ui/example_tab_browser.html`.
--
-- Attach this script to ONE entity. It spawns the markup, then on every
-- frame:
--   * L1 / R1 (with edge detection) cycle the active tab.
--   * Left stick Y (with deadzone + re-arm) steps the item cursor.
--   * The on_press names from the markup also work — clicking a tab button or
--     pressing A on the script's entity calls back here.

-- ===========================================================================
-- Spawn the UI on ready.
function on_ready()
    action("hui_spawn", { template = "ui/example_tab_browser.html" })
end

-- ===========================================================================
-- State the script keeps between frames. `_G` (Lua globals) survive across
-- on_update calls because each script keeps its own VM.

local tabs       = { "tab_loadout", "tab_perks", "tab_maps", "tab_career" }
local tab_count  = #tabs
local active_tab = 1   -- 1-based index into `tabs`

local item_count = 5
local cursor     = 0   -- 0-based index into the item_* nodes

-- Edge-detection state. `gamepad_l1` is just a per-frame bool; we want a
-- one-shot trigger, so we compare against the previous frame's value.
local prev_l1, prev_r1 = false, false

-- "Re-arm" the stick so a held direction only steps the cursor once.
local axis_armed = true

-- ===========================================================================
-- Colors. The pattern for setting a color on a markup entity by name is:
--   set_on("entity_name", "BackgroundColor.color", { r=..., g=..., b=..., a=... })
-- The values are 0..1 linear (Bevy `Srgba` parses 0..1).

local color_tab_active     = { r = 0.298, g = 0.545, b = 0.961, a = 1 }  -- #4C8BF5
local color_tab_idle       = { r = 0.067, g = 0.082, b = 0.110, a = 1 }  -- #11151C
local color_item_idle      = { r = 0.106, g = 0.122, b = 0.153, a = 1 }  -- #1B1F27
local color_item_idle_brd  = { r = 0.106, g = 0.122, b = 0.153, a = 1 }  -- #1B1F27
local color_item_focus     = { r = 0.133, g = 0.165, b = 0.212, a = 1 }  -- #222A36
local color_item_focus_brd = { r = 0.298, g = 0.545, b = 0.961, a = 1 }  -- #4C8BF5

-- ===========================================================================
-- Repaint everything from the current state. We call this whenever a tab or
-- cursor change happens — keeps the source of truth in Lua and the markup
-- entities as a derived view.

local function repaint()
    for i, name in ipairs(tabs) do
        local color = (i == active_tab) and color_tab_active or color_tab_idle
        set_on(name, "BackgroundColor.color", color)
    end
    for i = 0, item_count - 1 do
        local name = "item_" .. i
        if i == cursor then
            set_on(name, "BackgroundColor.color", color_item_focus)
            set_on(name, "BorderColor.top",    color_item_focus_brd)
            set_on(name, "BorderColor.right",  color_item_focus_brd)
            set_on(name, "BorderColor.bottom", color_item_focus_brd)
            set_on(name, "BorderColor.left",   color_item_focus_brd)
        else
            set_on(name, "BackgroundColor.color", color_item_idle)
            set_on(name, "BorderColor.top",    color_item_idle_brd)
            set_on(name, "BorderColor.right",  color_item_idle_brd)
            set_on(name, "BorderColor.bottom", color_item_idle_brd)
            set_on(name, "BorderColor.left",   color_item_idle_brd)
        end
    end
end

-- ===========================================================================
-- Per-frame input.

function on_update()
    -- L1 / R1 edge detection.
    if gamepad_l1 and not prev_l1 then
        active_tab = ((active_tab - 2) % tab_count) + 1
        repaint()
    end
    if gamepad_r1 and not prev_r1 then
        active_tab = (active_tab % tab_count) + 1
        repaint()
    end
    prev_l1, prev_r1 = gamepad_l1, gamepad_r1

    -- Left stick Y with re-arm: only step when the stick crosses the deadzone
    -- from neutral, then disarm until it returns to neutral.
    local y = gamepad_left_y or 0
    if math.abs(y) < 0.3 then
        axis_armed = true
    elseif axis_armed then
        if y > 0.5 then
            cursor = (cursor + 1) % item_count
            repaint()
        elseif y < -0.5 then
            cursor = (cursor - 1) % item_count
            repaint()
        end
        axis_armed = false
    end

    -- South face button (A on Xbox, X on PlayStation) confirms.
    if gamepad_south and not _G._prev_a then
        print("[tab_browser] confirm: tab=" .. tabs[active_tab] .. " cursor=" .. cursor)
    end
    _G._prev_a = gamepad_south
end

-- ===========================================================================
-- Mouse / direct clicks. Same handler, same UI — `on_press` names from the
-- markup land here.

function on_ui(name, args, entity)
    if name == "select_tab_loadout" then active_tab = 1; repaint()
    elseif name == "select_tab_perks"   then active_tab = 2; repaint()
    elseif name == "select_tab_maps"    then active_tab = 3; repaint()
    elseif name == "select_tab_career"  then active_tab = 4; repaint()
    end
end
