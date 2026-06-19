-- lab_063_camera_preset_switcher.lua
-- WHY: Bind named CameraPresets to number keys so a level can cut between framed
-- shots (overview, action, closeup). We only call goto_camera_preset on the key
-- edge, otherwise we'd re-issue the move every frame and fight the camera lerp.
-- Setup: define CameraPresets named below; press 1/2/3 or gamepad shoulders.

function props()
  return {
    presets = { "overview", "action", "closeup" },
    _index = 1,
  }
end

function on_update()
  -- Direct number-key selection.
  if is_key_just_pressed("Digit1") then jump_to(1) end
  if is_key_just_pressed("Digit2") then jump_to(2) end
  if is_key_just_pressed("Digit3") then jump_to(3) end

  -- Shoulder buttons cycle through the list.
  if gamepad_r1 and not p._r1_was then cycle(1) end
  if gamepad_l1 and not p._l1_was then cycle(-1) end
  p._r1_was = gamepad_r1
  p._l1_was = gamepad_l1
end

function jump_to(i)
  if i < 1 or i > #p.presets then return end
  p._index = i
  goto_camera_preset(p.presets[i])
  play_sound("camera_cut")
end

function cycle(dir)
  local n = #p.presets
  local i = ((p._index - 1 + dir) % n) + 1
  jump_to(i)
end
