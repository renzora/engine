-- lab_049_wheel_zoom.lua
-- Mouse-wheel zoom that drives a distance prop and repositions along local back.
-- WHY: we keep the zoom distance in a prop (clamped) and apply it by pushing the
-- entity straight back from the origin; the wheel just nudges that distance.
-- Setup: attach to a camera; scroll to zoom in/out within the clamp range.

function props()
  return {
    zoom = { value = 8.0, hint = "Current distance", tab = "Zoom" },
    min_zoom = { value = 2.0, hint = "Closest", tab = "Zoom" },
    max_zoom = { value = 20.0, hint = "Farthest", tab = "Zoom" },
    sensitivity = { value = 1.5, hint = "Units per wheel notch", tab = "Zoom" },
  }
end

function on_update()
  if mouse_scroll ~= 0 then
    -- Scroll up (positive) zooms in => smaller distance.
    zoom = clamp(zoom - mouse_scroll * sensitivity, min_zoom, max_zoom)
    -- Place along the world +Z axis at the new distance, looking at origin.
    set_position(position_x, position_y, zoom)
    look_at(0.0, 0.0, 0.0)
  end
end
