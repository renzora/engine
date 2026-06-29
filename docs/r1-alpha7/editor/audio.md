# Audio

Sound brings a game to life. In Renzora you can attach sounds to objects, play music and effects from your scripts, and balance everything in a friendly visual mixer — no audio engineering degree required.

This page walks you through the basics. When you need the deep technical details, the [Scripting API](/docs/r1-alpha5/api/scripting) has the full reference.

## How sound works

Audio plays both in the editor and in your exported game. Renzora can play these file types out of the box:

| Format | Use it for |
|--------|------------|
| `.ogg` | Music and long clips (small file size, streams from disk) |
| `.mp3` | Music (small file size, plays almost anywhere) |
| `.wav` | Sound effects (plays instantly, no delay) |
| `.flac` | High-quality source audio |

A simple rule of thumb: **OGG for music, WAV for sound effects.**

> **One thing to know:** audio works on Windows, Linux, and macOS, but not in the web (browser) build. If you export to Web, sounds, the recorder, and the mixer are turned off.

## Adding a sound to an object

To make an object in your scene play a sound, give it an **AudioPlayer** component.

In the editor:

1. Select your object in the scene.
2. In the Inspector, click **Add Component** and choose **AudioPlayer**.
3. Set the **Clip** field to your sound file (for example `audio/jump.wav`).
4. Turn on **Autoplay** if you want it to start the moment the game runs.

That's the whole setup for a basic sound. The most common settings you'll reach for:

| Setting | What it does |
|---------|--------------|
| **Clip** | The sound file to play |
| **Volume** | How loud it is (1.0 is normal) |
| **Pitch** | Higher = faster/squeakier, lower = slower/deeper |
| **Looping** | Repeat the clip over and over |
| **Autoplay** | Start automatically when the game runs |
| **Bus** | Which mixer channel it plays through (more on this below) |

There are more advanced options too — random clip pools, volume/pitch jitter, fades, and reverb. See the [Scripting API](/docs/r1-alpha5/api/scripting) for the complete list of fields.

### Making sound feel 3D

Turn on the **Spatial** option and a sound will come from the object's position in the world — louder up close, quieter far away. Great for campfires, machines, or chatting NPCs.

Set **Spatial Min Distance** to roughly the size of the thing making the sound (a campfire ~3 m, a whisper ~0.5 m), and **Spatial Max Distance** to how far it should still be heard.

> **Don't forget the ears!** Spatial sound is silent until something in the scene is listening. Add an **AudioListener** component to your camera (or your player). It is not added for you automatically.

## Playing sounds from a script

You can also trigger sounds with code. The same functions work in Lua, Rhai, and visual Blueprints, so use whichever you prefer.

```lua
function on_ready()
    play_music("audio/main_theme.ogg", 0.6, 1.5)  -- file, volume, fade-in seconds
end

function on_update()
    if is_key_just_pressed("Space") then
        play_sound("audio/jump.ogg")   -- play a quick sound effect
    end

    if is_key_just_pressed("Return") then
        play_audio()                   -- fire this object's AudioPlayer
    end
end
```

The handful of functions you'll use most:

| Function | What it does |
|----------|--------------|
| `play_sound(path)` | Play a one-shot sound effect |
| `play_music(path)` | Start a looping music track |
| `stop_music()` | Stop the music |
| `play_audio()` | Trigger this object's AudioPlayer (uses its 3D and random-clip settings) |

> Music does not crossfade — starting a new track stops the old one right away (with an optional fade-in).

Rhai can play audio too, but with simpler function signatures, and the keyboard-input functions shown above are Lua-only. For the full list of audio functions and the small Lua/Rhai differences, see the [Lua scripting guide](/docs/r1-alpha5/scripting/lua) and the [Scripting API](/docs/r1-alpha5/api/scripting).

## The mixer

The **Mixer** panel is your audio control board. Every sound flows through a "bus" — a channel you can adjust on its own — so you can turn music down without touching your sound effects, for example.

![The Mixer panel showing channel strips for the Master, SFX, Music, and Ambient buses, each with a volume fader, a round pan knob, and Mute (M) / Solo (S) buttons. A New bus box on the right adds custom channels.](/assets/previews/mixer.png)

Renzora starts you with four buses:

- **Master** — controls everything at once.
- **SFX** — your sound effects.
- **Music** — background music.
- **Ambient** — environmental loops like wind or rain.

On each channel strip you can drag the **fader** to set volume, turn the **Pan** knob to move it left or right, and use **M** (mute) and **S** (solo) to quickly silence or isolate a channel. Need more channels? Type a name in the **New bus** box on the right and click **Create**, then point an AudioPlayer's **Bus** field at it.

> Bus volumes are set here in the Mixer, not from scripts. Advanced users can also add effects (FX) to a bus and even pipe in a live microphone — see the [Scripting API](/docs/r1-alpha5/api/scripting) for those features.

## Recording and cinematics

Two more panels live alongside the mixer:

- **Record** — a timeline for recording and arranging audio clips.
- **Sequencer** — a cinematics tool for scripting camera moves and timed events, saved as a `.renseq` file.

These are optional tools for more advanced projects; you don't need them to add sound to a game.

## Tips

- **Use several clips for repeated sounds.** Footsteps and impacts sound more natural with a few variations and a little pitch/volume randomness.
- **OGG for music, WAV for sound effects.** OGG keeps music files small; WAV plays with zero delay.
- **Always add an AudioListener** to your camera, or 3D sound will be silent.
- **Pre-place your audio objects** with Autoplay off, then trigger them from a script to avoid hitches when the sound first loads.
