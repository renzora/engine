-- Gamepad Character Controller (with built-in third-person camera)
--
-- Drives an animated character AND its chase camera from one script, with a
-- full locomotion + action state machine layered on top.
--
--   Left stick / WASD   : move, relative to where the camera is facing.
--   Right stick / mouse : orbit the camera around the character.
--   South (A/Cross) / Space      : jump (real vertical arc; gravity brings you
--                                   back down — falling plays "Jumping_Down",
--                                   a heavy landing plays "Hard_Landing").
--   West (X/Square) / J          : punch.
--   North (Y/Triangle) / K       : kick.
--   East (B/Circle) / C          : toggle crouch / crawl.
--   L1 (LB) / Left Ctrl   (hold) : crouch / crawl.
--   L2 (LT) / right mouse (hold) : "strafe lock" — the character keeps facing
--                                   away from the camera so the stick selects
--                                   forward run / backward walk / side strafe
--                                   instead of just turning to face travel.
--
-- LOCOMOTION (grounded, free mode): Idle <-> Standard_Run; the character turns
-- to face travel directly from the stick. A "Push_Stop" skid plays if you stop
-- while holding Square/West.
--
-- VERTICAL MOTION / COLLISION: this controller is PHYSICS-DRIVEN. It applies
-- its own gravity into a vertical velocity and moves every frame through the
-- engine's collide-and-slide (`move_controller`), reading `grounded` back from
-- `PhysicsReadState`. That means the character stands on / slides against real
-- colliders (cubes, stacked geometry, world meshes) instead of a flat plane,
-- and "jumping down" works off any ledge. A fast landing plays the hard-landing
-- clip.
--
-- REQUIRED SETUP (in the editor, else the character won't collide and will just
-- fall):
--   * This character entity: add a Physics Body (Kinematic) + a Collision Shape
--     (Capsule sized to the model).
--   * Anything you want to stand on/bump into: add a Physics Body (Static) + a
--     Collision Shape (Box / Mesh).
--
-- Attach this to the CHARACTER entity (the one with the Animator). It controls
-- the camera by name via `set_on`, so set `camera_name` to your camera entity.

function props()
    return {
        -- Movement
        move_speed    = { type = "float",  value = 3.0,   hint = "Max move speed (world units/sec)" },
        crouch_speed  = { type = "float",  value = 1.4,   hint = "Move speed while crouched/crawling" },
        turn_speed    = { type = "float",  value = 540.0, hint = "How fast the character turns (deg/sec)" },
        deadzone      = { type = "float",  value = 0.2,   hint = "Stick magnitude below this is ignored" },
        facing_offset = { type = "float",  value = 0.0,   hint = "Add to facing yaw if the model faces the wrong way (try 180)" },

        -- Vertical motion / jumping
        gravity       = { type = "float",  value = 18.0,  hint = "Downward accel (units/sec^2)" },
        jump_speed    = { type = "float",  value = 6.0,   hint = "Initial upward velocity on jump" },
        fall_speed    = { type = "float",  value = 1.5,   hint = "Descend faster than this -> play the falling clip" },
        land_speed    = { type = "float",  value = 6.0,   hint = "Land faster than this -> play the hard-landing clip" },

        -- Animation: locomotion
        idle_anim     = { type = "string", value = "Idle",             hint = "Clip while standing still" },
        run_anim      = { type = "string", value = "Standard_Run",     hint = "Clip while moving forward" },
        walk_back_anim= { type = "string", value = "Walking_Backwards",hint = "Clip while moving backward (strafe lock)" },
        strafe_l_anim = { type = "string", value = "Walk_Strafe_Left", hint = "Clip while strafing left (strafe lock)" },
        strafe_r_anim = { type = "string", value = "Right_Shimmy",     hint = "Clip while strafing right (strafe lock)" },
        stop_anim     = { type = "string", value = "Push_Stop",        hint = "Skid clip when stopping while Square/West is held (empty = disabled)" },

        -- Animation: crouch / crawl
        crawl_anim    = { type = "string", value = "Crawling",         hint = "Clip while crouched and moving" },
        crouch_anim   = { type = "string", value = "Crawling",         hint = "Clip while crouched and still" },

        -- Animation: air. Rising alternatives in this set: "Jump", "Jump__1_".
        -- Falling alternatives: "Jumping_Down", "Jumping_Down__1_".
        jump_anim     = { type = "string", value = "Jump__1_",         hint = "Clip while rising (try Jump or Jump__1_)" },
        fall_anim     = { type = "string", value = "Jumping_Down",     hint = "Clip while falling (try Jumping_Down or Jumping_Down__1_)" },
        land_anim     = { type = "string", value = "Hard_Landing",     hint = "Clip on a heavy landing" },

        -- Animation: one-shot actions
        punch_anim    = { type = "string", value = "Punching",         hint = "Clip on punch (West / J)" },
        kick_anim     = { type = "string", value = "Mma_Kick",         hint = "Clip on kick (North / K)" },
        punch_time    = { type = "float",  value = 1.1,   hint = "How long the punch lasts (sec); stretches the clip" },
        kick_time     = { type = "float",  value = 1.3,   hint = "How long the kick lasts (sec); stretches the clip" },

        blend         = { type = "float",  value = 0.15,  hint = "Crossfade time between animations (sec)" },

        -- Camera
        camera_name   = { type = "string", value = "Camera", hint = "Name of the camera entity to control" },
        cam_distance  = { type = "float",  value = 4.5,   hint = "Camera distance from character" },
        cam_height    = { type = "float",  value = 1.2,   hint = "Look-at height above character origin" },
        cam_deadzone  = { type = "float",  value = 0.15,  hint = "Right-stick magnitude below this is ignored (stops orbit drift)" },
        orbit_speed   = { type = "float",  value = 150.0, hint = "Right-stick orbit speed (deg/sec)" },
        mouse_orbit   = { type = "float",  value = 0.2,   hint = "Mouse orbit sensitivity (fallback)" },
        follow_smooth = { type = "float",  value = 10.0,  hint = "Camera position smoothing (higher = snappier)" },
        pitch_min     = { type = "float",  value = -10.0, hint = "Lowest camera pitch (deg)" },
        pitch_max     = { type = "float",  value = 80.0,  hint = "Highest camera pitch (deg)" },
        invert_y      = { type = "bool",   value = false, hint = "Invert vertical camera control" },

        -- Internal state
        _yaw        = { type = "float",  value = 0.0,   hint = "Internal: character facing yaw (deg)" },
        _cam_yaw    = { type = "float",  value = 0.0,   hint = "Internal: camera orbit yaw (deg)" },
        _cam_pitch  = { type = "float",  value = 20.0,  hint = "Internal: camera orbit pitch (deg)" },
        _cam_x      = { type = "float",  value = 0.0,   hint = "Internal: smoothed camera x" },
        _cam_y      = { type = "float",  value = 0.0,   hint = "Internal: smoothed camera y" },
        _cam_z      = { type = "float",  value = 0.0,   hint = "Internal: smoothed camera z" },
        _state      = { type = "string", value = "",    hint = "Internal: current animation state" },
        _vy         = { type = "float",  value = 0.0,   hint = "Internal: vertical velocity" },
        _grounded   = { type = "bool",   value = true,  hint = "Internal: grounded last frame (for landing detection)" },
        _lock_anim  = { type = "string", value = "",    hint = "Internal: full-body action clip" },
        _lock_timer = { type = "float",  value = 0.0,   hint = "Internal: time left in action clip" },
        _stop_timer = { type = "float",  value = 0.0,   hint = "Internal: time left in push-stop clip" },
        _was_moving = { type = "bool",   value = false, hint = "Internal: was moving last frame" },
        _crouch_tgl = { type = "bool",   value = false, hint = "Internal: crouch toggled on (Circle)" },
        _prev_jump  = { type = "bool",   value = false, hint = "Internal: jump held last frame" },
        _prev_punch = { type = "bool",   value = false, hint = "Internal: punch held last frame" },
        _prev_kick  = { type = "bool",   value = false, hint = "Internal: kick held last frame" },
        _prev_crouch= { type = "bool",   value = false, hint = "Internal: crouch button held last frame" },
        _init       = { type = "bool",   value = false, hint = "Internal: camera position initialised" },
    }
end

function on_ready()
    _yaw        = rotation_y
    _state      = ""
    _vy         = 0.0
    _grounded   = true
    _lock_anim  = ""
    _lock_timer = 0.0
    _stop_timer = 0.0
    _was_moving = false
    _crouch_tgl = false
    _prev_jump  = false
    _prev_punch = false
    _prev_kick  = false
    _prev_crouch= false
    _init       = false
    lock_cursor()
end

-- Wrap an angle into [-180, 180].
local function wrap180(a)
    return (a + 180.0) % 360.0 - 180.0
end

-- Hamilton product of two quaternions {x,y,z,w}.
local function quat_mul(a, b)
    return {
        x = a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
        y = a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
        z = a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w,
        w = a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z,
    }
end

-- Crossfade into a clip only when the state actually changes.
local function set_anim(name, looping)
    if _state ~= name then
        crossfade_animation(name, blend, looping)
        _state = name
    end
end

-- Start a one-shot, full-body action that blocks movement until it finishes.
-- If `hold` is given, the clip is time-stretched (via playback speed) to last
-- exactly that many seconds — used to make short punch/kick clips read longer.
local function start_action(name, hold)
    crossfade_animation(name, blend, false)
    _state      = name
    _lock_anim  = name
    local len = get_animation_length(name)
    if len <= 0.0 then len = 0.6 end  -- fallback if length unknown
    if hold and hold > 0.0 then
        set_animation_speed(len / hold)
        _lock_timer = hold
    else
        _lock_timer = len
    end
end

-- Turn _yaw toward a target yaw at turn_speed.
local function turn_toward(target_yaw)
    local diff = wrap180(target_yaw - _yaw)
    local step = clamp(diff, -turn_speed * delta, turn_speed * delta)
    _yaw = wrap180(_yaw + step)
end

function on_update()
    -- ----- Camera orbit input (right stick, mouse fallback) -----
    local ox = gamepad_right_x
    local oy = gamepad_right_y
    -- Apply a deadzone so a resting (slightly drifting) stick doesn't keep
    -- slowly orbiting the camera. Below it, fall back to mouse look.
    if math.sqrt(ox * ox + oy * oy) < cam_deadzone then
        ox = mouse_delta_x * mouse_orbit * 0.5
        oy = -mouse_delta_y * mouse_orbit * 0.5
    else
        ox = ox * orbit_speed * delta
        oy = oy * orbit_speed * delta
    end

    _cam_yaw = wrap180(_cam_yaw - ox)
    local dy = oy
    if invert_y then dy = -dy end
    _cam_pitch = clamp(_cam_pitch - dy, pitch_min, pitch_max)

    if is_key_just_pressed("Escape") then unlock_cursor() end
    if mouse_left_just_pressed then lock_cursor() end

    -- ----- Movement input (left stick, WASD fallback) -----
    local mx = gamepad_left_x
    local my = gamepad_left_y        -- up = +1
    if is_key_pressed("KeyA") then mx = mx - 1.0 end
    if is_key_pressed("KeyD") then mx = mx + 1.0 end
    if is_key_pressed("KeyW") then my = my + 1.0 end
    if is_key_pressed("KeyS") then my = my - 1.0 end

    local in_mag = math.sqrt(mx * mx + my * my)
    if in_mag > 1.0 then mx = mx / in_mag; my = my / in_mag end

    -- Camera-relative ground directions from the orbit yaw.
    local ry = math.rad(_cam_yaw)
    local fwd_x, fwd_z     = -math.sin(ry), -math.cos(ry)  -- away from camera
    local right_x, right_z = math.cos(ry),  -math.sin(ry)  -- camera's right

    local dir_x = fwd_x * my + right_x * mx
    local dir_z = fwd_z * my + right_z * mx
    local move_mag = math.sqrt(dir_x * dir_x + dir_z * dir_z)
    local moving = move_mag > deadzone

    -- ----- Modifier / action buttons (gamepad buttons are level, track prev) -----
    local aim_held  = gamepad_left_trigger > 0.5 or mouse_right
    local jump_btn  = gamepad_south or is_key_pressed("Space")
    local punch_btn = gamepad_west  or is_key_pressed("KeyJ")
    local kick_btn  = gamepad_north or is_key_pressed("KeyK")
    local crouch_btn= gamepad_east  or is_key_pressed("KeyC")
    local jump_edge  = jump_btn   and not _prev_jump
    local punch_edge = punch_btn  and not _prev_punch
    local kick_edge  = kick_btn   and not _prev_kick
    local crouch_edge= crouch_btn and not _prev_crouch
    _prev_jump, _prev_punch, _prev_kick, _prev_crouch = jump_btn, punch_btn, kick_btn, crouch_btn

    -- Circle toggles crawl; L1 / Left Ctrl is a hold. Either keeps us crouched.
    if crouch_edge then _crouch_tgl = not _crouch_tgl end
    local crouch_held = _crouch_tgl or gamepad_l1 or is_key_pressed("ControlLeft")

    -- ----- Tick down timed clips -----
    if _lock_timer > 0.0 then _lock_timer = _lock_timer - delta end
    if _stop_timer > 0.0 then _stop_timer = _stop_timer - delta end
    local locked = _lock_timer > 0.0
    -- An action just finished: restore normal playback speed (it may have been
    -- stretched by start_action) before any locomotion clip resumes.
    if not locked and _lock_anim ~= "" then
        set_animation_speed(1.0)
        _lock_anim = ""
    end

    -- ----- Ground state (from physics: result of last move_controller slide) -----
    local grounded = get("PhysicsReadState.grounded") == true

    -- Landing: just touched down after a fast fall -> hard-landing clip.
    if grounded and not _grounded and _vy < -land_speed and not locked then
        start_action(land_anim)
        locked = true
    end

    -- ----- Start jump / actions (only when grounded and not mid-action) -----
    if not locked and grounded then
        if jump_edge then
            _vy = jump_speed
            grounded = false
        elseif punch_edge then
            start_action(punch_anim, punch_time); locked = true
        elseif kick_edge then
            start_action(kick_anim, kick_time); locked = true
        end
    end

    -- ----- Gravity -----
    -- On the ground we keep a small downward bias so the slide stays glued to
    -- the surface (and walks down small steps); in the air we accelerate down.
    if grounded and _vy <= 0.0 then
        _vy = -2.0
    else
        _vy = _vy - gravity * delta
    end

    -- ----- Facing + horizontal movement (blocked during an action) -----
    local dx, dz = 0.0, 0.0
    if not locked then
        if moving then
            local speed = move_speed
            if crouch_held and grounded then speed = crouch_speed end
            dx = dir_x * speed * delta
            dz = dir_z * speed * delta
            -- Strafe lock faces away from the camera; otherwise face travel.
            if aim_held and grounded then
                turn_toward(wrap180(math.deg(math.atan2(fwd_x, fwd_z)) + facing_offset))
            else
                turn_toward(math.deg(math.atan2(dir_x, dir_z)) + facing_offset)
            end
        end
        set_rotation(0.0, _yaw, 0.0)
    end

    -- Slide the full delta (horizontal + vertical) through the physics
    -- collide-and-slide: it stops us on top of / against colliders (cubes,
    -- world geometry) instead of clipping through, and refreshes `grounded`.
    move_controller(dx, _vy * delta, dz)

    -- ----- Pick the animation (highest priority first) -----
    if locked then
        -- Action clip is already playing; leave it be.
    elseif not grounded then
        -- Airborne: rising plays the jump clip, descending plays the fall clip.
        if _vy > fall_speed then
            set_anim(jump_anim, false)
        else
            set_anim(fall_anim, false)
        end
    elseif crouch_held then
        if moving then set_anim(crawl_anim, true) else set_anim(crouch_anim, true) end
    elseif aim_held then
        if moving then
            -- Pick run / back / strafe from the stick relative to the camera.
            if math.abs(my) >= math.abs(mx) then
                if my >= 0.0 then set_anim(run_anim, true)
                else              set_anim(walk_back_anim, true) end
            else
                if mx < 0.0 then set_anim(strafe_l_anim, true)
                else             set_anim(strafe_r_anim, true) end
            end
        else
            set_anim(idle_anim, true)
        end
    else
        -- Free locomotion.
        if moving then
            set_anim(run_anim, true)
        else
            -- Skid-stop only while Square/West is held. A plain stick release
            -- just goes to Idle (no skid).
            if stop_anim ~= "" and punch_btn and _was_moving and grounded and _stop_timer <= 0.0 then
                local len = get_animation_length(stop_anim)
                _stop_timer = (len > 0.0) and math.min(len, 0.5) or 0.4
            end
            if stop_anim ~= "" and punch_btn and _stop_timer > 0.0 then set_anim(stop_anim, false)
            else                                                       set_anim(idle_anim, true) end
        end
    end

    _was_moving = moving and grounded
    _grounded   = grounded   -- remember for next-frame landing detection

    -- ----- Drive the chase camera -----
    local fx = position_x
    local fy = position_y + cam_height
    local fz = position_z

    local rp = math.rad(_cam_pitch)
    local ch = math.cos(rp)
    local des_x = fx + math.sin(ry) * ch * cam_distance
    local des_y = fy + math.sin(rp) * cam_distance
    local des_z = fz + math.cos(ry) * ch * cam_distance

    if not _init then
        _cam_x, _cam_y, _cam_z = des_x, des_y, des_z
        _init = true
    else
        local t = 1.0 - math.exp(-follow_smooth * delta)
        _cam_x = _cam_x + (des_x - _cam_x) * t
        _cam_y = _cam_y + (des_y - _cam_y) * t
        _cam_z = _cam_z + (des_z - _cam_z) * t
    end

    set_on(camera_name, "Transform.translation.x", _cam_x)
    set_on(camera_name, "Transform.translation.y", _cam_y)
    set_on(camera_name, "Transform.translation.z", _cam_z)

    local qy = { x = 0.0, y = math.sin(ry * 0.5), z = 0.0, w = math.cos(ry * 0.5) }
    local qx = { x = math.sin(-rp * 0.5), y = 0.0, z = 0.0, w = math.cos(-rp * 0.5) }
    local q = quat_mul(qy, qx)
    set_on(camera_name, "Transform.rotation.x", q.x)
    set_on(camera_name, "Transform.rotation.y", q.y)
    set_on(camera_name, "Transform.rotation.z", q.z)
    set_on(camera_name, "Transform.rotation.w", q.w)
end
