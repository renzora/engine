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

**Where is it heard from?** By default, your game camera — you don't have to set anything up, and while you're editing it's the viewport camera, so you can fly around an emitter and hear it move.

Add an **AudioListener** component only when the ears belong somewhere other than the camera:

- **Third-person games** — the camera trails your character by a few metres, which puts their own footsteps in front of them and misjudges how close everything is. Put the listener on the character.
- **Strategy or top-down games** — a camera fifty metres up is past most Spatial Max Distances, so the scene fades out as you zoom. Put the listener nearer the action.
- **Split screen** — with more than one camera, this is how you say which one hears.

An AudioListener wins over the camera wherever it is. Untick **Active** to disable one without deleting it. None of this affects ordinary non-spatial sounds — those play at the volume and pan you set, wherever the listener is.

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

![The Mixer panel showing colour-coded channel strips for the SFX, Music, and Ambient buses, each topped by a coloured bar and holding a pan knob, a tall volume fader with a dB readout, and Mute (M) / Solo (S) buttons. A + tile adds custom channels; Master sits apart on the right.](/assets/previews/mixer.png)

Renzora starts you with four buses:

- **Master** — controls everything at once. It sits apart on the right, because everything else feeds into it.
- **SFX** — your sound effects.
- **Music** — background music.
- **Ambient** — environmental loops like wind or rain.

On each channel strip you can drag the **fader** to set volume (the number underneath is the gain in dB), turn the **Pan** knob to move it left or right, and use **M** (mute) and **S** (solo) to quickly silence or isolate a channel.

### Laying the board out

A slim strip across the top of the panel sets the board's shape. Nothing here changes your sound — it's only about how much room each channel gets and which way the channels run. The three keys sit at the right-hand end, out of the way of the channels; hover any of them for a tooltip.

| Key | What it does |
|---|---|
| **Compact** (arrows pointing in) | Narrower strips: a smaller pan knob and no "Pan" caption, so more channels fit at once |
| **Wide** (arrows pointing out) | Roomier strips, the default — a bigger pan knob that's easier to aim at |
| **Columns / rows** | Flips the channels between standing columns and stacked rows. The icon shows the layout you're currently in |

**Vertical** is the classic mixing desk: strips stand up side by side, and the fader is as tall as the panel allows — the longer a fader, the more precisely you can set it. This is the default.

**Horizontal** lays each channel down as a row instead: the name on the left, then the pan knob, a fader and level meter lying on their sides, the dB readout and the M/S keys. It fits many more channels in a panel that's wide but short — which is the shape the Mixer usually ends up when it's docked at the bottom of the editor — and the list of rows scrolls when it outgrows the panel.

Narrow the panel and a row gives ground in a fixed order: the name column shrinks first, then the fader and meter. The controls stay inside the strip's frame rather than sliding off the edge of the panel, so mute and solo remain reachable at any width you can dock the Mixer to.

Master keeps its place either way: at the far right in vertical, at the bottom in horizontal, always behind a dividing rule, because everything else feeds into it.

### Adding and naming your own buses

Click the **+** tile at the end of the row and you get a new channel immediately, already named (`Bus 1`, `Bus 2`, …) and already given a colour. Point an AudioPlayer's **Bus** field at it and it's wired up.

To give it a better name, **double-click the name at the top of the strip**. An edit box opens over the strip — type over it and press **Enter** (or click away) to keep the change, **Escape** to abandon it.

Renaming is always safe, and always has been free of consequences you can't see. A bus has a permanent **key** (the `Bus 1` it was born with) that never changes, and a **name** that's just a label. Sound is routed by the key, so renaming a bus can't strand an AudioPlayer pointing at it — including ones in scenes you don't currently have open. The Bus field on an AudioPlayer shows the key for that reason.

The four built-in buses can't be renamed — `Master`, `SFX`, `Music` and `Ambient` are the keys the engine routes by, so they're fixed.

### Your mixer is saved with the project

The board is stored in your project's `project.toml`, so bus volumes, panning, mute/solo, colours and every custom channel you created come back when you reopen the project — and, more importantly, **ship with your game**. An exported game builds the same board before the first scene loads, so an AudioPlayer routed to `Footsteps` plays on `Footsteps` and not on some fallback. Saving happens on its own as you work; there's nothing to press.

### Colour-coding a channel

Each strip carries a colour, shown as a bar across the top of the strip and a matching tint on its frame, so you can pick a channel out of a crowded board at a glance. New buses take the next colour in the palette automatically.

**Right-click any strip** to open its menu and pick a different colour from the swatch grid. The strip's current colour is the one ringed in white.

### The strip menu

Right-clicking a strip is also how you reach everything that isn't a live control — a strip is under a hundred pixels wide, so there is no room for it on the strip itself. The menu opens at your cursor and re-positions itself to stay on screen, so it works the same on a strip at the edge of the panel as one in the middle.

| Menu entry | What it does |
|---|---|
| **Rename** | Same inline edit as double-clicking the name (custom buses only) |
| **Colour** | The swatch grid described above |
| **Input device** | Capture a live microphone into this bus |
| **Output device** | Which device the bus plays out of |
| **Delete bus** | Removes a custom bus (custom buses only) |

Devices are listed fresh each time you open the menu, so a microphone you plugged in after starting the editor shows up straight away.

> Bus volumes are set here in the Mixer, not from scripts. Advanced users can also add effects (FX) to a bus — see the [Scripting API](/docs/r1-alpha5/api/scripting) for those features.

## Recording and cinematics

Two more panels live alongside the mixer:

- **Record** — a timeline for recording and arranging audio clips.

These are optional tools for more advanced projects; you don't need them to add sound to a game.

## Tips

- **Use several clips for repeated sounds.** Footsteps and impacts sound more natural with a few variations and a little pitch/volume randomness.
- **OGG for music, WAV for sound effects.** OGG keeps music files small; WAV plays with zero delay.
- **You don't need an AudioListener** unless the ears belong somewhere other than the camera — third person, a pulled-back strategy camera, or split screen.
- **Pre-place your audio objects** with Autoplay off, then trigger them from a script to avoid hitches when the sound first loads.
