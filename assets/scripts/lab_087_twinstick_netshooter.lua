-- lab_087_twinstick_netshooter.lua
-- Twin-stick shooter: move with left stick, aim/fire with right, bullets sync.
-- WHY: aim direction comes from the right stick angle, and each shot is both
-- spawned locally AND broadcast via rpc so remote peers see your bullets. We
-- gate firing on a cooldown and edge-detect the trigger so holding it fires at
-- a fixed cadence. Setup: attach to the player pawn; needs a gamepad.
-- Multiplayer: connect first; on_rpc spawns remote bullets owned by sender.

local SPEED = 6.0
local COOLDOWN = 0.18
local cd = 0
local shot = 0

local function spawn_bullet(x, z, dx, dz, tag)
  shot = shot + 1
  local nm = "blt_" .. tag .. "_" .. shot
  spawn_primitive(nm, "sphere", x, 1.0, z, 1.0, 0.9, 0.2)
  -- Give it velocity so the physics layer carries it forward.
  set_on(nm, "Transform.scale.x", 0.3)
end

function on_update()
  local dt = delta
  cd = math.max(0, cd - dt)

  -- Movement from the left stick.
  local mx = gamepad_left_x or 0
  local mz = gamepad_left_y or 0
  if math.abs(mx) > 0.15 or math.abs(mz) > 0.15 then
    move_controller(mx * SPEED * dt, 0, mz * SPEED * dt)
  end

  -- Aim from the right stick; fire when its magnitude crosses a deadzone.
  local ax = gamepad_right_x or 0
  local az = gamepad_right_y or 0
  local mag = math.sqrt(ax * ax + az * az)
  if mag > 0.3 then
    local yaw = math.deg(math.atan(ax, -az))
    set_rotation(0, yaw, 0)
    if cd <= 0 then
      cd = COOLDOWN
      local dx, dz = ax / mag, az / mag
      spawn_bullet(position_x + dx, position_z + dz, dx, dz, "me")
      play_sound("sfx/shoot.ogg")
      if net_is_connected() then
        rpc("shoot", { x = position_x, z = position_z, dx = dx, dz = dz })
      end
    end
  end
end

function on_rpc(name, args, from)
  if name == "shoot" and from ~= 0 then
    spawn_bullet(args.x + args.dx, args.z + args.dz, args.dx, args.dz, "p" .. from)
  end
end
