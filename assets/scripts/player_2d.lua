-- 2D Player + Follow Camera
--
-- Top-down / platformer style character controller for a 2D scene.
-- Attach this to the SPRITE entity you want to control.
--
-- The camera is referenced by its TAG (set the "Tag" field on your Camera 2D
-- entity in the inspector, e.g. "camera"). Tags are the reliable identifier —
-- entity Names can be duplicated and a fresh 2D camera is named "Camera 2D",
-- not "Camera". (A Name works too: lookup tries tags first, then Names.)
--
-- WASD or the arrow keys move the sprite; a left gamepad stick works too.
-- In a 2D scene X is horizontal and Y is vertical, so movement lives on the
-- XY plane and Z is left alone (it's the sprite's draw depth / sort layer).
--
-- NOTE: the amber "camera boundary" in the 2D viewport does NOT move — it is
-- the fixed project window-area frame, anchored at world origin, not the
-- camera. The follow is visible when you press PLAY: the engine drives the
-- render camera from your Camera 2D's transform, so the view scrolls with the
-- player. Watch the sprites move under a stationary boundary, not the boundary.
--
-- Centering: a 2D camera uses a top-left viewport origin, so its position maps
-- to the top-left of the view, not the middle. To keep the sprite centered we
-- offset the camera by half the visible world extent, which is the project
-- resolution (project_width/height globals) scaled by the camera's zoom.

function props()
    return {
        speed         = { type = "float",  value = 300.0,   hint = "Move speed (pixels/sec)" },
        camera_tag    = { type = "string", value = "camera", hint = "Tag of the Camera 2D entity to follow with" },
        follow_smooth = { type = "float",  value = 8.0,      hint = "Camera catch-up speed (higher = snappier, 0 = locked)" },
        zoom          = { type = "float",  value = 1.0,      hint = "Camera's orthographic scale (match the Camera 2D's zoom)" },
        use_gamepad   = { type = "bool",   value = true,     hint = "Also drive movement from the left stick" },
        _warned       = { type = "bool",   value = false,    hint = "Internal: missing-camera warning shown" },
    }
end

function on_update()
    ----------------------------------------------------------------
    -- MOVE THE SPRITE
    ----------------------------------------------------------------
    -- input_x / input_y already combine WASD + arrows and are normalized
    -- to unit length on the diagonals, so straight and diagonal movement
    -- feel the same speed.
    local mx = input_x
    local my = input_y

    -- Fold in the left stick. Stick + keys share the same axes, so we take
    -- whichever pushed harder this frame rather than adding them (which would
    -- let a half-held stick + a key exceed full speed). Bevy reports stick Y
    -- as up-positive, matching input_y (W = +1), so no sign flip.
    if use_gamepad then
        if math.abs(gamepad_left_x) > math.abs(mx) then mx = gamepad_left_x end
        if math.abs(gamepad_left_y) > math.abs(my) then my = gamepad_left_y end
    end

    translate(mx * speed * delta, my * speed * delta, 0)

    ----------------------------------------------------------------
    -- FOLLOW CAMERA
    ----------------------------------------------------------------
    -- Read back the sprite's post-move position so the camera chases the
    -- value it actually landed on this frame.
    local px = get("Transform.translation.x")
    local py = get("Transform.translation.y")
    if px == nil or py == nil then return end

    -- The camera is looked up by tag (falls back to Name). If nothing matches,
    -- warn once so a wrong/blank tag is obvious instead of failing silently.
    local cx = get_on(camera_tag, "Transform.translation.x")
    local cy = get_on(camera_tag, "Transform.translation.y")
    if cx == nil or cy == nil then
        if not _warned then
            print("player_2d: no entity with tag '" .. camera_tag ..
                  "' — set that tag on your Camera 2D entity, or change camera_tag.")
            _warned = true
        end
        return
    end

    -- Where the camera must sit for the sprite to land dead-center. The 2D
    -- camera's top-left origin means its position is the view's top-left corner,
    -- so we pull it back by half the visible extent: +x/2 left, -y/2 down (world
    -- Y points up). project_width/height are the game resolution in world units.
    local half_w = project_width * zoom * 0.5
    local half_h = project_height * zoom * 0.5
    local target_x = px - half_w
    local target_y = py + half_h

    -- Exponential smoothing toward that target. Framerate-independent because
    -- the blend factor is derived from delta; follow_smooth <= 0 hard-locks
    -- the camera to the player with no lag.
    local t = 1.0
    if follow_smooth > 0.0 then
        t = 1.0 - math.exp(-follow_smooth * delta)
    end

    -- Only X and Y — leave the camera's Z (its 2D depth) exactly as authored.
    set_on(camera_tag, "Transform.translation.x", cx + (target_x - cx) * t)
    set_on(camera_tag, "Transform.translation.y", cy + (target_y - cy) * t)
end
