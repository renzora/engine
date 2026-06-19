-- lab_020_key_toggle_vis: Space toggles the entity's visibility.
-- We track our own bool and flip it on the just-pressed edge, because reading a
-- held key would toggle every frame the key is down and produce a flicker.

local visible = true

function props()
    return {
        toggle_key = { value = "Space", hint = "Bevy key name to toggle", tab = "Input" },
    }
end

function on_update()
    if is_key_just_pressed(toggle_key) then
        visible = not visible
        set_visibility(visible)
    end
end
