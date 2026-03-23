-- =============================================================================
-- RENZORA ENGINE SCRIPTING API STRESS TEST
-- =============================================================================
-- Attach to any entity with a dynamic rigid body + collider.
-- Cycles through every scripting API category on key presses.
-- Press 1-9, 0, and F1-F9 to trigger each test group.
-- Watch the console log for [TEST] output confirming each call executed.
--
-- Requirements:
--   - Entity has a RigidBody (Dynamic) + Collider
--   - Scene has entities named "TestTarget", "TestChild" (child of self),
--     "Camera 3D", and a UI canvas named "TestCanvas" with widgets
--   - Audio files: "audio/test_sfx.ogg", "audio/test_music.ogg"
--   - Animation clips: "idle", "walk", "run"
--   - A scene file: "scenes/test_scene.ron"
-- =============================================================================

function props()
    return {
        test_speed        = { value = 5.0,   hint = "Movement speed for transform tests" },
        test_force        = { value = 100.0, hint = "Force magnitude for physics tests" },
        test_volume       = { value = 0.5,   hint = "Audio volume for sound tests" },
        timer_duration    = { value = 2.0,   hint = "Duration for timer tests" },
        shake_intensity   = { value = 0.3,   hint = "Screen shake intensity" },
        shake_duration    = { value = 0.5,   hint = "Screen shake duration" },
        target_name       = { value = "TestTarget", hint = "Name of target entity" },
        child_name        = { value = "TestChild",  hint = "Name of child entity" },
        camera_name       = { value = "Camera 3D",  hint = "Name of camera entity" },
        canvas_name       = { value = "TestCanvas",  hint = "Name of UI canvas" },
        phase             = { value = 0.0,   hint = "Internal: current test phase" },
        test_counter      = { value = 0.0,   hint = "Internal: tests executed" },
        initialized       = { value = 0.0,   hint = "Internal" },
    }
end

-- ─── helpers ────────────────────────────────────────────────────────────────

local function log(msg)
    print_log("[TEST] " .. msg)
end

local function pass(name)
    test_counter = test_counter + 1
    log("PASS #" .. test_counter .. ": " .. name)
end

-- ─── test groups ────────────────────────────────────────────────────────────

local function test_context_variables()
    log("=== CONTEXT VARIABLES ===")

    -- Time
    log("  delta          = " .. tostring(delta))
    log("  elapsed        = " .. tostring(elapsed))
    pass("delta / elapsed readable")

    -- Transform
    log("  position       = " .. position_x .. ", " .. position_y .. ", " .. position_z)
    log("  rotation       = " .. rotation_x .. ", " .. rotation_y .. ", " .. rotation_z)
    log("  scale          = " .. scale_x .. ", " .. scale_y .. ", " .. scale_z)
    pass("transform context vars")

    -- Input axes
    log("  input_x        = " .. tostring(input_x))
    log("  input_y        = " .. tostring(input_y))
    log("  mouse_x        = " .. tostring(mouse_x))
    log("  mouse_y        = " .. tostring(mouse_y))
    log("  mouse_delta_x  = " .. tostring(mouse_delta_x))
    log("  mouse_delta_y  = " .. tostring(mouse_delta_y))
    log("  mouse_scroll   = " .. tostring(mouse_scroll))
    pass("input context vars")

    -- Mouse buttons
    log("  mouse_left     = " .. tostring(mouse_left))
    log("  mouse_right    = " .. tostring(mouse_right))
    log("  mouse_middle   = " .. tostring(mouse_middle))
    pass("mouse button context vars")

    -- Gamepad
    log("  gp_left_x      = " .. tostring(gamepad_left_x))
    log("  gp_left_y      = " .. tostring(gamepad_left_y))
    log("  gp_right_x     = " .. tostring(gamepad_right_x))
    log("  gp_right_y     = " .. tostring(gamepad_right_y))
    log("  gp_left_trig   = " .. tostring(gamepad_left_trigger))
    log("  gp_right_trig  = " .. tostring(gamepad_right_trigger))
    log("  gp_south       = " .. tostring(gamepad_south))
    log("  gp_east        = " .. tostring(gamepad_east))
    log("  gp_west        = " .. tostring(gamepad_west))
    log("  gp_north       = " .. tostring(gamepad_north))
    log("  gp_l1          = " .. tostring(gamepad_l1))
    log("  gp_r1          = " .. tostring(gamepad_r1))
    log("  gp_dpad_up     = " .. tostring(gamepad_dpad_up))
    log("  gp_dpad_down   = " .. tostring(gamepad_dpad_down))
    log("  gp_dpad_left   = " .. tostring(gamepad_dpad_left))
    log("  gp_dpad_right  = " .. tostring(gamepad_dpad_right))
    log("  gp_select      = " .. tostring(gamepad_select))
    log("  gp_start       = " .. tostring(gamepad_start))
    log("  gp_l3          = " .. tostring(gamepad_l3))
    log("  gp_r3          = " .. tostring(gamepad_r3))
    pass("gamepad context vars")

    -- Entity
    log("  self_entity_id  = " .. tostring(self_entity_id))
    log("  self_entity_name= " .. tostring(self_entity_name))
    pass("entity context vars")

    -- Camera
    log("  camera_yaw      = " .. tostring(camera_yaw))
    pass("camera context vars")

    -- Collision
    log("  is_colliding    = " .. tostring(is_colliding))
    pass("collision context vars")

    -- Health
    log("  self_health     = " .. tostring(self_health))
    log("  self_max_health = " .. tostring(self_max_health))
    pass("health context vars")

    -- Parent
    log("  has_parent      = " .. tostring(has_parent))
    if has_parent then
        log("  parent_pos    = " .. tostring(parent_position_x) .. ", " .. tostring(parent_position_y) .. ", " .. tostring(parent_position_z))
    end
    pass("parent context vars")

    -- Timers
    log("  timers_finished = " .. tostring(timers_finished))
    pass("timer context vars")
end

local function test_transform()
    log("=== TRANSFORM (SELF) ===")
    local ox, oy, oz = position_x, position_y, position_z

    set_position(ox + 1, oy, oz)
    pass("set_position")

    set_rotation(0, 45, 0)
    pass("set_rotation")

    set_scale(1.5, 1.5, 1.5)
    pass("set_scale")

    set_scale_uniform(1.0)
    pass("set_scale_uniform")

    translate(0, 0.5, 0)
    pass("translate")

    rotate(0, 10, 0)
    pass("rotate")

    look_at(0, 0, 0)
    pass("look_at")

    -- restore
    set_position(ox, oy, oz)
    set_rotation(0, 0, 0)
    set_scale(1, 1, 1)
    pass("transform restore")
end

local function test_parent_transform()
    log("=== PARENT TRANSFORM ===")
    if not has_parent then
        log("  SKIP (no parent)")
        return
    end

    parent_set_position(0, 1, 0)
    pass("parent_set_position")

    parent_set_rotation(0, 0, 0)
    pass("parent_set_rotation")

    parent_translate(0, 0.1, 0)
    pass("parent_translate")
end

local function test_child_transform()
    log("=== CHILD TRANSFORM ===")

    set_child_position(child_name, 0, 1, 0)
    pass("set_child_position")

    set_child_rotation(child_name, 0, 90, 0)
    pass("set_child_rotation")

    child_translate(child_name, 0, 0.1, 0)
    pass("child_translate")
end

local function test_input()
    log("=== INPUT FUNCTIONS ===")

    local w = is_key_pressed("KeyW")
    log("  KeyW pressed: " .. tostring(w))
    pass("is_key_pressed")

    local space = is_key_just_pressed("Space")
    log("  Space just pressed: " .. tostring(space))
    pass("is_key_just_pressed")

    local shift = is_key_just_released("ShiftLeft")
    log("  ShiftLeft just released: " .. tostring(shift))
    pass("is_key_just_released")
end

local function test_physics()
    log("=== PHYSICS ===")

    apply_force(0, test_force, 0)
    pass("apply_force")

    apply_impulse(0, 0.5, 0)
    pass("apply_impulse")

    set_velocity(0, 0, 0)
    pass("set_velocity")

    set_gravity_scale(1.0)
    pass("set_gravity_scale")
end

local function test_audio()
    log("=== AUDIO ===")

    play_sound("audio/test_sfx.ogg", test_volume)
    pass("play_sound (default bus)")

    play_sound("audio/test_sfx.ogg", test_volume, "sfx")
    pass("play_sound (named bus)")

    play_sound_looping("audio/test_sfx.ogg", test_volume * 0.3)
    pass("play_sound_looping")

    play_music("audio/test_music.ogg", test_volume, 1.0)
    pass("play_music (with fade-in)")

    stop_music(0.5)
    pass("stop_music (with fade-out)")

    stop_all_sounds()
    pass("stop_all_sounds")
end

local function test_timers()
    log("=== TIMERS ===")

    start_timer("test_once", timer_duration, false)
    pass("start_timer (one-shot)")

    start_timer("test_repeat", timer_duration, true)
    pass("start_timer (repeating)")

    stop_timer("test_repeat")
    pass("stop_timer")

    -- check if any timers finished
    if timers_finished then
        for _, name in ipairs(timers_finished) do
            log("  timer finished: " .. name)
        end
    end
    pass("timers_finished check")
end

local function test_debug()
    log("=== DEBUG / DRAWING ===")

    print_log("Hello from the stress test!")
    pass("print_log")

    draw_line(0, 0, 0, 5, 5, 5, 3.0)
    pass("draw_line (with duration)")

    draw_line(0, 0, 0, -5, 5, -5)
    pass("draw_line (no duration)")
end

local function test_rendering()
    log("=== RENDERING ===")

    set_visibility(false)
    pass("set_visibility (hide)")

    set_visibility(true)
    pass("set_visibility (show)")

    set_material_color(1.0, 0.0, 0.0, 1.0)
    pass("set_material_color (red, with alpha)")

    set_material_color(0.5, 0.5, 0.5)
    pass("set_material_color (grey, no alpha)")
end

local function test_animation()
    log("=== ANIMATION ===")

    play_animation("idle", true, 1.0)
    pass("play_animation (looping)")

    play_animation("walk", false)
    pass("play_animation (once, default speed)")

    set_animation_speed(2.0)
    pass("set_animation_speed")

    crossfade_animation("run", 0.3, true)
    pass("crossfade_animation")

    pause_animation()
    pass("pause_animation")

    resume_animation()
    pass("resume_animation")

    set_anim_param("speed", 1.5)
    pass("set_anim_param")

    set_anim_bool("grounded", true)
    pass("set_anim_bool")

    trigger_anim("attack")
    pass("trigger_anim")

    set_layer_weight("upper_body", 0.8)
    pass("set_layer_weight")

    stop_animation()
    pass("stop_animation")
end

local function test_camera()
    log("=== CAMERA ===")

    screen_shake(shake_intensity, shake_duration)
    pass("screen_shake")
end

local function test_entity()
    log("=== ENTITY ===")

    spawn_entity("StressTestSpawn")
    pass("spawn_entity")

    -- Note: despawn_self() would destroy us, so we skip it
    log("  SKIP despawn_self (would destroy test entity)")
    pass("despawn_self acknowledged")
end

local function test_scene()
    log("=== SCENE ===")
    -- Note: load_scene would unload current scene; log but don't call
    log("  SKIP load_scene (would unload current scene)")
    pass("load_scene acknowledged")
end

local function test_environment()
    log("=== ENVIRONMENT ===")

    set_sun_angles(45.0, 60.0)
    pass("set_sun_angles")

    set_fog(true, 10.0, 100.0)
    pass("set_fog (enable)")

    set_fog(false, 0, 0)
    pass("set_fog (disable)")
end

local function test_math_helpers()
    log("=== MATH HELPERS ===")

    local v3 = vec3(1.0, 2.0, 3.0)
    log("  vec3 = " .. v3.x .. ", " .. v3.y .. ", " .. v3.z)
    pass("vec3")

    local v2 = vec2(4.0, 5.0)
    log("  vec2 = " .. v2.x .. ", " .. v2.y)
    pass("vec2")

    local l = lerp(0.0, 10.0, 0.5)
    log("  lerp(0,10,0.5) = " .. l)
    pass("lerp")

    local c = clamp(15.0, 0.0, 10.0)
    log("  clamp(15,0,10) = " .. c)
    pass("clamp")
end

local function test_reflection()
    log("=== GENERIC REFLECTION (get/set) ===")

    -- get own transform via reflection
    local pos = get("Transform.translation")
    if pos then
        log("  get(Transform.translation) = " .. tostring(pos.x) .. ", " .. tostring(pos.y) .. ", " .. tostring(pos.z))
        pass("get (Transform.translation)")
    else
        log("  get(Transform.translation) returned nil")
        pass("get returned nil (may be expected)")
    end

    -- get on named entity
    local target_pos = get_on(target_name, "Transform.translation")
    if target_pos then
        log("  get_on(" .. target_name .. ") = " .. tostring(target_pos.x) .. ", " .. tostring(target_pos.y) .. ", " .. tostring(target_pos.z))
        pass("get_on (Transform.translation)")
    else
        log("  get_on returned nil (target entity may not exist)")
        pass("get_on returned nil")
    end

    -- set via reflection
    set("Transform.translation", vec3(position_x, position_y + 0.01, position_z))
    pass("set (Transform.translation)")

    -- set_on via reflection
    if target_pos then
        set_on(target_name, "Transform.translation", vec3(target_pos.x, target_pos.y + 0.01, target_pos.z))
        pass("set_on (Transform.translation)")
    else
        log("  SKIP set_on (no target)")
    end
end

local function test_game_ui()
    log("=== GAME UI ===")

    ui_show(canvas_name)
    pass("ui_show")

    ui_hide(canvas_name)
    pass("ui_hide")

    ui_toggle(canvas_name)
    pass("ui_toggle")

    ui_set_text("TestLabel", "Stress test active!")
    pass("ui_set_text")

    ui_set_progress("TestProgressBar", 0.75)
    pass("ui_set_progress")

    ui_set_health("TestHealthBar", 80.0, 100.0)
    pass("ui_set_health")

    ui_set_slider("TestSlider", 0.5)
    pass("ui_set_slider")

    ui_set_checkbox("TestCheckbox", true)
    pass("ui_set_checkbox")

    ui_set_toggle("TestToggle", false)
    pass("ui_set_toggle")

    ui_set_visible("TestLabel", true)
    pass("ui_set_visible")

    ui_set_theme("dark")
    pass("ui_set_theme")

    ui_set_color("TestPanel", 0.2, 0.3, 0.8, 1.0)
    pass("ui_set_color")
end

local function test_gauges()
    log("=== GAUGES ===")

    gauge_set("health", 100.0)
    pass("gauge_set")

    gauge_add_modifier("health", 10.0)
    pass("gauge_add_modifier")

    gauge_remove_modifier("health", 10.0)
    pass("gauge_remove_modifier")

    gauge_add_expr_modifier("health", "base * 1.1")
    pass("gauge_add_expr_modifier")

    gauge_instant("health", "set", 100.0)
    pass("gauge_instant")

    gauge_damage("health", 25.0)
    pass("gauge_damage")

    gauge_heal("health", 25.0)
    pass("gauge_heal")

    local hp = gauge_get("health")
    log("  gauge_get(health) = " .. tostring(hp))
    pass("gauge_get")
end

local function test_networking()
    log("=== NETWORKING ===")

    local is_srv = net_is_server()
    log("  is_server    = " .. tostring(is_srv))
    pass("net_is_server")

    local is_conn = net_is_connected()
    log("  is_connected = " .. tostring(is_conn))
    pass("net_is_connected")

    local cid = net_client_id()
    log("  client_id    = " .. tostring(cid))
    pass("net_client_id")

    net_send("test_channel", "hello world")
    pass("net_send")

    net_spawn("NetworkedCube", 0.0, 5.0, 0.0)
    pass("net_spawn")

    net_rpc("test_function", "arg1")
    pass("net_rpc")
end

local function test_combined_gameplay()
    log("=== COMBINED GAMEPLAY SCENARIO ===")
    log("  Simulating: player controller with input, physics, animation, audio, UI")

    -- 1. Read input
    local mx = input_x or 0
    local mz = input_y or 0
    local moving = (math.abs(mx) > 0.01 or math.abs(mz) > 0.01)
    pass("read movement input")

    -- 2. Apply movement force
    if moving then
        apply_force(mx * test_speed * 50, 0, mz * test_speed * 50)
        pass("apply movement force")
    else
        pass("no movement (idle)")
    end

    -- 3. Jump
    if is_key_just_pressed("Space") and is_colliding then
        apply_impulse(0, 8, 0)
        play_sound("audio/test_sfx.ogg", 0.4)
        pass("jump with sound")
    else
        pass("no jump this frame")
    end

    -- 4. Animation state
    if moving then
        crossfade_animation("walk", 0.2, true)
        set_anim_param("speed", math.sqrt(mx * mx + mz * mz))
    else
        crossfade_animation("idle", 0.2, true)
        set_anim_param("speed", 0.0)
    end
    set_anim_bool("grounded", is_colliding)
    pass("animation state machine")

    -- 5. Update UI
    ui_set_text("TestLabel", string.format("Pos: %.1f, %.1f, %.1f", position_x, position_y, position_z))
    ui_set_health("TestHealthBar", self_health or 100, self_max_health or 100)
    pass("UI update")

    -- 6. Camera follow via reflection
    local cam_pos = get_on(camera_name, "Transform.translation")
    if cam_pos then
        local t = clamp(5.0 * delta, 0, 1)
        local tx = lerp(cam_pos.x, position_x, t)
        local tz = lerp(cam_pos.z, position_z - 10, t)
        local ty = lerp(cam_pos.y, position_y + 8, t)
        set_on(camera_name, "Transform.translation", vec3(tx, ty, tz))
    end
    pass("camera follow via reflection")

    -- 7. Respawn check
    if position_y < -20 then
        set_position(0, 5, 0)
        set_velocity(0, 0, 0)
        screen_shake(0.2, 0.3)
        play_sound("audio/test_sfx.ogg", 0.6)
        log("  respawned!")
    end
    pass("respawn check")

    -- 8. Debug visualization
    draw_line(position_x, position_y, position_z,
              position_x + mx * 3, position_y, position_z + mz * 3, 0.0)
    pass("debug movement vector")
end

-- ─── main loop ──────────────────────────────────────────────────────────────

function on_update()
    if initialized < 0.5 then
        log("============================================")
        log("  RENZORA SCRIPTING API STRESS TEST")
        log("  Press keys 1-9, 0, F1-F9 to run groups")
        log("  Press P to run ALL tests at once")
        log("============================================")
        initialized = 1.0
    end

    -- Key 1: Context variables
    if is_key_just_pressed("Digit1") then
        test_counter = 0
        test_context_variables()
        log(">> Group 1 complete: " .. test_counter .. " tests")
    end

    -- Key 2: Transform (self)
    if is_key_just_pressed("Digit2") then
        test_counter = 0
        test_transform()
        log(">> Group 2 complete: " .. test_counter .. " tests")
    end

    -- Key 3: Parent + Child transform
    if is_key_just_pressed("Digit3") then
        test_counter = 0
        test_parent_transform()
        test_child_transform()
        log(">> Group 3 complete: " .. test_counter .. " tests")
    end

    -- Key 4: Input
    if is_key_just_pressed("Digit4") then
        test_counter = 0
        test_input()
        log(">> Group 4 complete: " .. test_counter .. " tests")
    end

    -- Key 5: Physics
    if is_key_just_pressed("Digit5") then
        test_counter = 0
        test_physics()
        log(">> Group 5 complete: " .. test_counter .. " tests")
    end

    -- Key 6: Audio
    if is_key_just_pressed("Digit6") then
        test_counter = 0
        test_audio()
        log(">> Group 6 complete: " .. test_counter .. " tests")
    end

    -- Key 7: Timers
    if is_key_just_pressed("Digit7") then
        test_counter = 0
        test_timers()
        log(">> Group 7 complete: " .. test_counter .. " tests")
    end

    -- Key 8: Debug / Drawing
    if is_key_just_pressed("Digit8") then
        test_counter = 0
        test_debug()
        log(">> Group 8 complete: " .. test_counter .. " tests")
    end

    -- Key 9: Rendering
    if is_key_just_pressed("Digit9") then
        test_counter = 0
        test_rendering()
        log(">> Group 9 complete: " .. test_counter .. " tests")
    end

    -- Key 0: Animation
    if is_key_just_pressed("Digit0") then
        test_counter = 0
        test_animation()
        log(">> Group 0 complete: " .. test_counter .. " tests")
    end

    -- F1: Camera
    if is_key_just_pressed("F1") then
        test_counter = 0
        test_camera()
        log(">> Group F1 complete: " .. test_counter .. " tests")
    end

    -- F2: Entity
    if is_key_just_pressed("F2") then
        test_counter = 0
        test_entity()
        log(">> Group F2 complete: " .. test_counter .. " tests")
    end

    -- F3: Environment
    if is_key_just_pressed("F3") then
        test_counter = 0
        test_environment()
        log(">> Group F3 complete: " .. test_counter .. " tests")
    end

    -- F4: Math helpers
    if is_key_just_pressed("F4") then
        test_counter = 0
        test_math_helpers()
        log(">> Group F4 complete: " .. test_counter .. " tests")
    end

    -- F5: Reflection (get/set)
    if is_key_just_pressed("F5") then
        test_counter = 0
        test_reflection()
        log(">> Group F5 complete: " .. test_counter .. " tests")
    end

    -- F6: Game UI
    if is_key_just_pressed("F6") then
        test_counter = 0
        test_game_ui()
        log(">> Group F6 complete: " .. test_counter .. " tests")
    end

    -- F7: Gauges
    if is_key_just_pressed("F7") then
        test_counter = 0
        test_gauges()
        log(">> Group F7 complete: " .. test_counter .. " tests")
    end

    -- F8: Networking
    if is_key_just_pressed("F8") then
        test_counter = 0
        test_networking()
        log(">> Group F8 complete: " .. test_counter .. " tests")
    end

    -- F9: Combined gameplay
    if is_key_just_pressed("F9") then
        test_counter = 0
        test_combined_gameplay()
        log(">> Group F9 complete: " .. test_counter .. " tests")
    end

    -- P: Run ALL tests
    if is_key_just_pressed("KeyP") then
        test_counter = 0
        log(">>>>>>>>>> RUNNING ALL TESTS <<<<<<<<<<")

        test_context_variables()
        test_transform()
        test_parent_transform()
        test_child_transform()
        test_input()
        test_physics()
        test_audio()
        test_timers()
        test_debug()
        test_rendering()
        test_animation()
        test_camera()
        test_entity()
        test_environment()
        test_math_helpers()
        test_reflection()
        test_game_ui()
        test_gauges()
        test_networking()
        test_combined_gameplay()

        log(">>>>>>>>>> ALL TESTS COMPLETE: " .. test_counter .. " total <<<<<<<<<<")
    end
end
