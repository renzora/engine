-- Gentle camera sway.
--
-- A slow, non-repeating drift for an idle or cinematic camera — reads as a boat
-- deck at low intensity and a handheld shot at high. Written for looking at the
-- ocean, where a perfectly still camera is the thing that gives the water away.
--
-- Every channel is the sum of TWO sine waves whose frequencies share no common
-- multiple. One sine is a metronome: the eye locks onto the period within a few
-- seconds. Two incommensurate ones take hours to line up again, which is what
-- makes the motion read as drift rather than animation.
--
-- FOV is part of it twice over. The lens breathes as a seventh channel, and the
-- rotation amplitudes are divided by it: a 0.8 degree roll is a shrug at 90
-- degrees of FOV and a lurch at 25, so holding the *angle* constant makes the
-- sway feel completely different on every lens. Scaling by FOV holds the
-- on-screen motion constant instead, which is what "gentle" actually means.
--
-- The script OWNS this entity's transform and its FOV — it writes absolute
-- position, rotation and field of view every frame, relative to the pose
-- captured when it became ready. Do not attach it to a camera something else is
-- also driving (an FPS controller, a follow rig, an aim-down-sights zoom); it
-- will fight them. For that case, apply the offsets with `translate`/`rotate`
-- deltas instead of `set_position`/`set_rotation`.

local TAU = math.pi * 2.0

-- Rest pose, captured in on_ready. Sway is an offset from here, so the camera
-- always returns to the position you actually placed it at.
local base_x, base_y, base_z = 0.0, 0.0, 0.0
local base_pitch, base_yaw, base_roll = 0.0, 0.0, 0.0
local base_fov = 0.0
-- The lens the reference amplitudes were tuned against. Anything narrower sways
-- proportionally less in degrees to cover the same arc of screen.
local REFERENCE_FOV = 60.0
local t = 0.0

function props()
    return {
        intensity = {
            value = 1.0,
            hint = "Master multiplier over every amplitude below. 0 = perfectly still",
            tab = "Sway",
        },
        speed = {
            value = 1.0,
            hint = "Time scale. Below 1 reads as heavier, calmer water",
            tab = "Sway",
        },
        heave = { value = 0.10, hint = "Rise and fall, metres", tab = "Position" },
        surge = { value = 0.06, hint = "Fore/aft drift, metres", tab = "Position" },
        drift = { value = 0.08, hint = "Left/right drift, metres", tab = "Position" },
        pitch_angle = { value = 0.45, hint = "Nose up/down, degrees", tab = "Rotation" },
        roll_angle = { value = 0.80, hint = "Horizon tilt, degrees", tab = "Rotation" },
        yaw_angle = { value = 0.30, hint = "Heading drift, degrees", tab = "Rotation" },
        fov_breath = {
            value = 0.40,
            hint = "Lens breathing, degrees of FOV. 0 leaves the lens alone",
            tab = "Lens",
        },
        fov_compensate = {
            value = true,
            hint = "Scale the rotation amplitudes by FOV so the on-screen sway "
                .. "looks the same on any lens",
            tab = "Lens",
        },
    }
end

function on_ready()
    t = 0.0
    base_x, base_y, base_z = position_x, position_y, position_z
    base_pitch, base_yaw, base_roll = rotation_x, rotation_y, rotation_z
    -- 0 means an orthographic camera (or no projection yet). Falling back to
    -- the reference lens keeps the angular scaling at 1x rather than dividing
    -- by zero, and the breathing simply has nothing to write.
    base_fov = camera_fov()
    if base_fov <= 0.0 then
        base_fov = REFERENCE_FOV
    end
end

--- Two sines at incommensurate frequencies, weighted 2:1 so the slower one
--- carries the motion and the faster one keeps it from ever repeating.
local function wave(f1, f2, phase)
    return math.sin(t * f1 * TAU + phase) * 0.66
        + math.sin(t * f2 * TAU + phase * 1.7) * 0.34
end

function on_update()
    t = t + delta * speed

    -- Frequencies in Hz. Ocean swell heaves at roughly 0.1–0.2 Hz, so the
    -- position channels sit there. The rotation channels run a little faster:
    -- a hull tips as it meets a wave face slightly before it rises, and
    -- borrowing that ordering is most of what sells the motion.
    local offset_up = wave(0.11, 0.17, 0.0) * heave
    local offset_right = wave(0.09, 0.15, 1.3) * drift
    local offset_forward = wave(0.07, 0.13, 2.7) * surge

    -- Angles are authored against a 60 degree lens; a narrower one needs
    -- proportionally smaller angles to sweep the same fraction of the frame.
    local lens = 1.0
    if fov_compensate then
        lens = base_fov / REFERENCE_FOV
    end

    local pitch = wave(0.13, 0.21, 0.6) * pitch_angle * lens
    local roll = wave(0.08, 0.19, 2.1) * roll_angle * lens
    local yaw = wave(0.05, 0.11, 4.2) * yaw_angle * lens

    -- The lens breathes on its own slow pair, unrelated to the rest, so a zoom
    -- never lines up with a pitch and read as one deliberate move.
    local breath = wave(0.06, 0.09, 3.4) * fov_breath

    -- Drift along the camera's own axes rather than the world's, so "forward"
    -- means into the view no matter which way the camera was left pointing.
    -- Yaw is measured from the rest pose: the sway's own yaw is far too small
    -- to be worth feeding back in, and using it would couple the channels.
    local yaw_rad = math.rad(base_yaw)
    local sin_yaw, cos_yaw = math.sin(yaw_rad), math.cos(yaw_rad)
    -- Bevy cameras look down local -Z.
    local world_x = offset_right * cos_yaw - offset_forward * sin_yaw
    local world_z = -offset_right * sin_yaw - offset_forward * cos_yaw

    set_position(
        base_x + world_x * intensity,
        base_y + offset_up * intensity,
        base_z + world_z * intensity
    )
    set_rotation(
        base_pitch + pitch * intensity,
        base_yaw + yaw * intensity,
        base_roll + roll * intensity
    )
    if fov_breath ~= 0.0 and base_fov > 0.0 then
        set_fov(base_fov + breath * intensity)
    end
end
