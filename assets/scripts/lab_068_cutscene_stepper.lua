-- lab_068_cutscene_stepper.lua
-- WHY: A linear cutscene sequencer. Each beat sets up its action then arms a
-- one-shot timer; when the timer fires we advance the step index. Driving it off
-- an index + timer (rather than sleeping) keeps it frame-accurate and skippable.
-- Setup: define CameraPresets used below; attach to a director entity.

function props()
  return {
    _step = 0,
    _running = false,
  }
end

function on_ready()
  advance()  -- start the show
end

function on_update()
  -- Allow skipping straight to the end.
  if is_key_just_pressed("Escape") then
    finish()
    return
  end
  for _, n in ipairs(timers_finished) do
    if n == "beat" then advance() end
  end
end

-- Each case = one beat: do something, then wait N seconds.
function advance()
  p._step = p._step + 1
  if p._step == 1 then
    goto_camera_preset("establishing"); play_music("cutscene_theme"); wait(3.0)
  elseif p._step == 2 then
    goto_camera_preset("hero_closeup"); play_sound("hero_line"); wait(4.0)
  elseif p._step == 3 then
    goto_camera_preset("villain_closeup"); play_sound("villain_line"); wait(4.0)
  elseif p._step == 4 then
    goto_camera_preset("wide_action"); action("ui_set_text", { name = "Subtitle", text = "The battle begins!" }); wait(2.5)
  else
    finish()
  end
end

function wait(secs)
  start_timer("beat", secs, false)
end

function finish()
  stop_music()
  action("ui_set_text", { name = "Subtitle", text = "" })
  goto_camera_preset("gameplay")
end
