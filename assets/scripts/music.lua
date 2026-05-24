-- Background Music
--
-- Plays a looping music track when the game starts. Attach this to ANY entity
-- (it doesn't need the AudioPlayer component — playback is driven from script).
--
-- `clip` is a path RELATIVE TO THE PROJECT FOLDER. e.g. if your file is at
--   <project>/audio/theme.ogg   then set clip = "audio/theme.ogg".
--
-- Music streams from disk, loops automatically, and routes to the "Music"
-- mixer bus (so the mixer's Music fader controls it). Supported formats:
-- .ogg (recommended for music), .wav, .flac, .mp3.

function props()
    return {
        clip          = { type = "string", value = "audio/background_music.mp3", hint = "Music file, relative to the project folder" },
        volume        = { type = "float",  value = 0.7, hint = "0..2 (1 = unity)" },
        fade_in       = { type = "float",  value = 1.5, hint = "Fade-in seconds (0 = instant)" },
        play_on_start = { type = "bool",   value = true, hint = "Start the music when the game begins" },
        _started      = { type = "bool",   value = false, hint = "Internal: music already started" },
    }
end

function on_ready()
    _started = false
    if play_on_start and clip ~= "" then
        play_music(clip, volume, fade_in)   -- loops automatically
        _started = true
    end
end
