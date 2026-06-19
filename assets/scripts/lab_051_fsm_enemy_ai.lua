-- lab_051_fsm_enemy_ai.lua
-- WHY: A finite-state enemy. Keeping the current state in a string prop means
-- the FSM survives reloads and is inspectable in the editor. Transitions are
-- driven purely by distance to the player's last-known position.
-- Setup: attach to an enemy entity; set props.home_x/z to its patrol anchor.

function props()
  return {
    state = "idle",       -- idle | patrol | chase
    home_x = 0.0,
    home_z = 0.0,
    patrol_radius = 4.0,
    sight_range = 8.0,
    speed = 2.0,
    _idle_timer = 0.0,
    _patrol_target_x = 0.0,
    _patrol_target_z = 0.0,
  }
end

function on_ready()
  -- Seed the first patrol point so chase->patrol has somewhere to go.
  p._patrol_target_x = p.home_x + p.patrol_radius
  p._patrol_target_z = p.home_z
  play_animation("idle", true)
end

local function dist_xz(ax, az, bx, bz)
  local dx, dz = ax - bx, az - bz
  return math.sqrt(dx * dx + dz * dz)
end

function on_update()
  -- The "player" position is read via reflection off a known entity name.
  local px = get_on("Player", "Transform.translation.x") or position_x
  local pz = get_on("Player", "Transform.translation.z") or position_z
  local to_player = dist_xz(position_x, position_z, px, pz)

  if p.state == "idle" then
    p._idle_timer = p._idle_timer + delta
    if to_player < p.sight_range then
      p.state = "chase"; crossfade_animation("run", 0.2, true)
    elseif p._idle_timer > 2.0 then
      p._idle_timer = 0.0
      -- pick a new patrol target on the ring
      local a = elapsed * 1.3
      p._patrol_target_x = p.home_x + math.cos(a) * p.patrol_radius
      p._patrol_target_z = p.home_z + math.sin(a) * p.patrol_radius
      p.state = "patrol"; crossfade_animation("walk", 0.25, true)
    end
  elseif p.state == "patrol" then
    if to_player < p.sight_range then
      p.state = "chase"; crossfade_animation("run", 0.2, true)
    else
      move_towards(p._patrol_target_x, p._patrol_target_z, p.speed * 0.5)
      if dist_xz(position_x, position_z, p._patrol_target_x, p._patrol_target_z) < 0.5 then
        p.state = "idle"; crossfade_animation("idle", 0.25, true)
      end
    end
  elseif p.state == "chase" then
    if to_player > p.sight_range * 1.5 then
      p.state = "idle"; crossfade_animation("idle", 0.3, true)
    else
      move_towards(px, pz, p.speed)
    end
  end
end

-- Steers + faces a world-space target on the XZ plane.
function move_towards(tx, tz, spd)
  local dx, dz = tx - position_x, tz - position_z
  local len = math.sqrt(dx * dx + dz * dz)
  if len > 0.001 then
    translate((dx / len) * spd * delta, 0.0, (dz / len) * spd * delta)
    look_at(tx, position_y, tz)
  end
end
