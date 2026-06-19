-- lab_074_billboard_camera.lua
-- WHY: A Y-axis billboard (sprite/health-bar/grass) that always faces the camera.
-- Using camera_yaw directly is cheaper and steadier than look_at on the camera
-- position, and constraining to yaw keeps the quad upright instead of tilting.
-- Setup: attach to a flat quad/plane facing +Z in its local space.

function props()
  return {
    yaw_offset = 180.0,    -- flip if the quad's front faces -Z
    bob = false,
    bob_height = 0.1,
    _base_y = 0.0,
  }
end

function on_ready()
  p._base_y = position_y
end

function on_update()
  -- camera_yaw is radians; convert and apply so the quad's normal faces the cam.
  local yaw = math.deg(camera_yaw) + p.yaw_offset
  set_rotation(0.0, yaw, 0.0)

  -- Optional idle bob for pickups / floating labels.
  if p.bob then
    set_position(position_x, p._base_y + math.sin(elapsed * 2.0) * p.bob_height, position_z)
  end
end
