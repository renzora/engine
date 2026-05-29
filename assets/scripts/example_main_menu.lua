-- Example: gamepad/keyboard-driven main menu.
-- Pair with `templates/example_main_menu.html` + `templates/example_settings.html`
-- (or `ui/...` if you keep the engine convention).
--
-- Attach to ONE entity. Spawns the menu on ready, navigates with the left
-- stick, D-pad, or arrow keys, confirms with South (A / Cross) or Enter.
-- Mouse hover follows the same cursor state so all three input methods agree.

local TEMPLATE_MENU     = "templates/example_main_menu.html"
local TEMPLATE_SETTINGS = "templates/example_settings.html"

-- Audio assets. Asset-server-relative; supply your own if these paths don't
-- exist in your project.
local SFX_MOVE    = "audio/menu_button.mp3"
local SFX_CONFIRM = "audio/confirm.mp3"
local MUSIC_MENU  = "audio/main_music.mp3"

local VOL_MOVE    = 0.5
local VOL_CONFIRM = 0.8
local VOL_MUSIC   = 0.5

-- Flip this if pushing "up" on your left stick moves the cursor DOWN instead
-- of up. (Bevy reports stick Y in different signs depending on the driver.)
local INVERT_Y = false

-- Re-arm threshold — stick must return below this to allow the next step.
-- Step threshold — stick must exceed this (after re-arm) to step the cursor.
local AXIS_DEADZONE = 0.25
local AXIS_STEP     = 0.5

function on_ready()
    action("hui_spawn", { template = TEMPLATE_MENU })
    play_music(MUSIC_MENU, VOL_MUSIC, 0.6)
end

-- =====================================================================
-- State
local items     = { "btn_play", "btn_settings", "btn_credits", "btn_exit" }
local cursor    = 1
local on_settings = false
local menu_hidden = false

local prev_a          = false
local prev_b          = false   -- East / Circle = back
local prev_start      = false
local prev_dpad_up    = false
local prev_dpad_down  = false
local axis_armed      = true

local color_focus = { r = 0.298, g = 0.545, b = 0.961, a = 1 }   -- #4C8BF5
local color_idle  = { r = 0.106, g = 0.122, b = 0.153, a = 1 }   -- #1B1F27

-- Repaint the focus border to match `cursor`. Bevy 0.18 splits border colors
-- per-side, so we set all four.
local function repaint()
    for i, name in ipairs(items) do
        local c = (i == cursor) and color_focus or color_idle
        set_on(name, "BorderColor.top",    c)
        set_on(name, "BorderColor.right",  c)
        set_on(name, "BorderColor.bottom", c)
        set_on(name, "BorderColor.left",   c)
    end
end

local function move_down()
    cursor = (cursor % #items) + 1
    repaint()
    play_sound(SFX_MOVE, VOL_MOVE)
end

local function move_up()
    cursor = ((cursor - 2) % #items) + 1
    repaint()
    play_sound(SFX_MOVE, VOL_MOVE)
end

local function confirm()
    local choice = items[cursor]
    print("[main_menu] confirm -> " .. choice)
    play_sound(SFX_CONFIRM, VOL_CONFIRM)

    if choice == "btn_play" then
        action("hui_hide", { name = "main_menu_root" })
        menu_hidden = true
        stop_music(0.5)
    elseif choice == "btn_settings" then
        action("hui_hide",  { name = "main_menu_root" })
        action("hui_spawn", { template = TEMPLATE_SETTINGS })
        on_settings = true
    elseif choice == "btn_credits" then
        print("[main_menu] (credits would open here)")
    elseif choice == "btn_exit" then
        action("quit")
    end
end

function on_update()
    -- Escape / Start / Circle (East) always reopens the menu.
    local start_edge = gamepad_start and not prev_start
    local east_edge  = gamepad_east  and not prev_b
    prev_start = gamepad_start
    prev_b     = gamepad_east
    if is_key_just_pressed("Escape") or start_edge or east_edge then
        if on_settings then
            action("hui_despawn", { name = "settings_root" })
            on_settings = false
        end
        if menu_hidden then
            -- Coming back from the hidden "in-game" state — bring music back too.
            play_music(MUSIC_MENU, VOL_MUSIC, 0.4)
        end
        menu_hidden = false
        action("hui_show", { name = "main_menu_root" })
        repaint()
        play_sound(SFX_MOVE, VOL_MOVE)
        return
    end

    if on_settings or menu_hidden then return end

    -- Left stick Y with deadzone + re-arm.
    local y = gamepad_left_y or 0
    if INVERT_Y then y = -y end
    if math.abs(y) < AXIS_DEADZONE then
        axis_armed = true
    elseif axis_armed then
        if y > AXIS_STEP then
            move_down()
        elseif y < -AXIS_STEP then
            move_up()
        end
        axis_armed = false
    end

    -- D-pad up/down (edge-detected).
    local dp_down_edge = gamepad_dpad_down and not prev_dpad_down
    local dp_up_edge   = gamepad_dpad_up   and not prev_dpad_up
    prev_dpad_down = gamepad_dpad_down
    prev_dpad_up   = gamepad_dpad_up
    if dp_down_edge then move_down() end
    if dp_up_edge   then move_up()   end

    -- Arrow keys.
    if is_key_just_pressed("ArrowDown") then move_down() end
    if is_key_just_pressed("ArrowUp")   then move_up()   end

    -- Confirm: gamepad South or Enter or Space.
    local south_edge = gamepad_south and not prev_a
    prev_a = gamepad_south
    if south_edge or is_key_just_pressed("Enter") or is_key_just_pressed("Space") then
        confirm()
    end
end

function on_ui(name, args, entity)
    -- Click on a button.
    if     name == "press_play"     then cursor = 1; repaint(); confirm()
    elseif name == "press_settings" then cursor = 2; repaint(); confirm()
    elseif name == "press_credits"  then cursor = 3; repaint(); confirm()
    elseif name == "press_exit"     then cursor = 4; repaint(); confirm()

    -- Mouse hover — moves the cursor without confirming, so the focus border
    -- follows the mouse. We only play the move SFX when the cursor actually
    -- changes; otherwise wiggling the mouse across the same button retriggers
    -- it every frame and the audio stutters.
    elseif name == "hover_btn_play"     then if cursor ~= 1 then cursor = 1; repaint(); play_sound(SFX_MOVE, VOL_MOVE) end
    elseif name == "hover_btn_settings" then if cursor ~= 2 then cursor = 2; repaint(); play_sound(SFX_MOVE, VOL_MOVE) end
    elseif name == "hover_btn_credits"  then if cursor ~= 3 then cursor = 3; repaint(); play_sound(SFX_MOVE, VOL_MOVE) end
    elseif name == "hover_btn_exit"     then if cursor ~= 4 then cursor = 4; repaint(); play_sound(SFX_MOVE, VOL_MOVE) end

    elseif name == "settings_back" then
        action("hui_despawn", { name = "settings_root" })
        action("hui_show",    { name = "main_menu_root" })
        on_settings = false
    end
end
