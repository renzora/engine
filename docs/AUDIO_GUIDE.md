# Audio Guide

A complete guide to the Renzora Engine audio system — components, the mixer, spatial audio, scripting, effects sends, and the editor preview workflow.

The engine uses **Kira** as its audio backend, providing low-latency playback with per-sound control over volume, pitch, panning, looping, fading, and 3D spatial positioning. Audio is fully integrated with the ECS, the inspector, the scripting system, and the mixer panel.

---

## Table of Contents

1. [How Audio Works in the Engine](#how-audio-works-in-the-engine)
2. [Audio Player Component](#audio-emitter-component)
3. [Audio Listener Component](#audio-listener-component)
4. [Mixer Panel](#mixer-panel)
5. [Spatial Audio](#spatial-audio)
6. [Effects — Reverb & Delay Sends](#effects--reverb--delay-sends)
7. [Editor Preview](#editor-preview)
8. [Play Mode & Audio Lifecycle](#play-mode--audio-lifecycle)
9. [Scripting API — Sound Effects](#scripting-api--sound-effects)
10. [Scripting API — 3D Spatial Sounds](#scripting-api--3d-spatial-sounds)
11. [Scripting API — Music](#scripting-api--music)
12. [Scripting API — Playback Control](#scripting-api--playback-control)
13. [Scripting API — Volume & Pitch](#scripting-api--volume--pitch)
14. [Scripting API — Querying Sound State](#scripting-api--querying-sound-state)
15. [Scripting API — Component Properties](#scripting-api--component-properties)
16. [Bus Routing](#bus-routing)
17. [Supported Formats](#supported-formats)
18. [Recipes](#recipes)
19. [Architecture Overview](#architecture-overview)
20. [Best Practices](#best-practices)
21. [Troubleshooting](#troubleshooting)

---

## How Audio Works in the Engine

Audio in Renzora follows a **component-then-play** pattern:

1. **Design time**: You add `Audio Player` and `Audio Listener` components to entities in the editor. These are data components that describe what sound an entity produces and which entity is the listener.
2. **Play mode**: When you press `F5`, emitters with **Autoplay** enabled start playing immediately. Scripts can trigger additional sounds at any time via the audio API. The mixer applies volume, panning, mute, and solo to each bus in real-time.
3. **Stop**: When you exit play mode, all sounds stop instantly. Active sound handles, spatial tracks, and the listener are cleaned up. Everything resets.

Outside play mode, you can still **preview** sounds directly in the inspector via the play button on any Audio Player.

---

## Audio Player Component

Add an **Audio Player** to any entity to make it a sound source. This is the primary component for all audio playback.

### Properties

| Property | Type | Range | Default | Description |
|----------|------|-------|---------|-------------|
| **Clip** | String | file path | (empty) | Relative path to the audio file from the project root (e.g. `assets/sounds/hit.wav`) |
| **Volume** | Float | 0.0–2.0 | 1.0 | Linear amplitude. 1.0 = unity gain, 2.0 = double volume (+6 dB) |
| **Pitch** | Float | 0.1–4.0 | 1.0 | Playback speed multiplier. 0.5 = half speed (octave down), 2.0 = double speed (octave up) |
| **Panning** | Float | -1.0–1.0 | 0.0 | Stereo position. -1.0 = hard left, 0.0 = center, 1.0 = hard right |
| **Looping** | Bool | on/off | off | Loop the clip continuously |
| **Loop Start** | Float | 0.0+ seconds | 0.0 | Where the loop region begins (only visible when Looping is on) |
| **Loop End** | Float | 0.0+ seconds | 0.0 | Where the loop region ends. 0 = loop to end of file |
| **Autoplay** | Bool | on/off | off | Automatically start playing when entering play mode |
| **Fade In** | Float | 0.0–10.0 seconds | 0.0 | Fade-in duration when the sound starts |
| **Bus** | Dropdown | Master/Sfx/Music/Ambient/custom | Sfx | Which mixer bus to route the sound to |
| **Spatial** | Bool | on/off | off | Enable 3D positional audio based on the entity's transform |
| **Min Distance** | Float | 0.01–1000.0 | 1.0 | Distance at which spatial attenuation begins (only when Spatial is on) |
| **Max Distance** | Float | 0.1–10000.0 | 50.0 | Distance at which the sound is fully attenuated (only when Spatial is on) |
| **Rolloff** | Dropdown | Logarithmic/Linear | Logarithmic | Distance attenuation curve (only when Spatial is on) |
| **Reverb Send** | Float | 0.0–1.0 | 0.0 | How much signal is sent to the global reverb effect |
| **Delay Send** | Float | 0.0–1.0 | 0.0 | How much signal is sent to the global delay effect |

### Property Notes

- **Clip** paths are relative to your project root. Both forward and back slashes work.
- **Volume** above 1.0 amplifies the signal — useful for quiet source files, but can cause clipping.
- **Pitch** changes both speed and pitch together (like a turntable). 1.0 = original.
- **Panning** is overridden by spatial positioning when Spatial is enabled — the engine computes stereo pan from the listener-to-emitter direction.
- **Loop Start/End** allow seamless music loops that skip intro sections. Set Loop Start to the bar where the loop should restart.
- **Bus** determines which mixer channel strip controls the sound. The default "Sfx" bus is appropriate for most game sounds.
- **Reverb/Delay Send** values represent the send level (0.0 = no effect, 1.0 = full wet signal). The dry signal always plays through the bus at full volume.

---

## Audio Listener Component

Add an **Audio Listener** to the entity that represents the player's "ears" — typically the camera or the player character.

### Properties

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| **Active** | Bool | true | Whether this entity is the active listener for spatial audio |

### How It Works

- The engine uses the **first active** Audio Listener it finds each frame.
- The listener's `GlobalTransform` (position + rotation) determines how spatial sounds are panned and attenuated.
- Sounds in front of the listener are centered; sounds to the left/right are panned accordingly.
- Only **one** listener should be active at a time. If you have multiple, deactivate all but one.

### Typical Setup

| Scenario | Attach Listener To |
|----------|-------------------|
| First-person game | The camera entity |
| Third-person game | The player character entity (not the camera — you want sounds relative to the character) |
| Top-down game | The camera entity (spatial audio has less impact, but still useful for L/R panning) |
| VR | The head/camera rig |

---

## Mixer Panel

Open the **Mixer** panel from the panel menu. It provides a DAW-style mixing console with real-time control over all audio buses.

### Channel Strips

Each bus gets its own channel strip with:

| Control | Interaction | Description |
|---------|-------------|-------------|
| **Fader** | Drag up/down, click to jump, double-click to reset to 0 dB | Volume control (0 to +3.5 dB range) |
| **Pan Knob** | Drag left/right, double-click to center | Stereo positioning for the entire bus |
| **M (Mute)** | Click to toggle | Silences the bus completely |
| **S (Solo)** | Click to toggle | When any bus is soloed, only soloed buses are audible |
| **VU Meter** | Read-only | Segmented LED meter showing real-time audio activity |
| **dB Readout** | Read-only | Shows current fader value in decibels |

### Built-in Buses

| Bus | Color | Purpose |
|-----|-------|---------|
| **Master** | White | Controls overall output volume. All buses feed through Master. |
| **SFX** | Orange | Sound effects — footsteps, explosions, UI clicks, impacts |
| **Music** | Purple | Background music, streaming tracks |
| **Ambient** | Green | Environmental loops — wind, rain, crowd noise |

### Custom Buses

Click **+ Bus** to create additional buses with custom names. Useful for:
- Dialogue (separate from SFX for accessibility)
- Vehicle sounds (grouped for ducking)
- UI sounds (independent volume control)

Custom buses appear to the right of the built-in buses and have the same full set of controls.

### Mute & Solo Logic

- **Mute** silences only that bus.
- **Solo** is exclusive — when any bus has Solo active, all non-soloed buses go silent (except Master).
- Solo is useful for isolating a specific bus to debug audio issues.
- Mute and Solo interact: a muted bus stays silent even if soloed.

### VU Meter Zones

| Zone | Color | Level | Meaning |
|------|-------|-------|---------|
| Lower | Green | Safe | Normal operating range |
| Middle | Yellow | Warm | Approaching peak levels |
| Upper | Red | Hot | Risk of distortion |

The unity line (0 dB) is marked on both the fader and the meter. The fader goes up to +3.5 dB for headroom.

### Volume Persistence

Mixer volumes (master, sfx, music, ambient) are automatically saved to your user config and restored on the next launch. Adjustments persist across sessions.

---

## Spatial Audio

Spatial audio makes sounds feel like they come from specific positions in 3D space. Sounds get louder as you approach, quieter as you move away, and pan left/right based on direction.

### How It Works

1. The **Audio Listener** component marks the entity that acts as the "ears" (usually the camera or player).
2. **Audio Player** components with **Spatial** enabled are positioned in 3D space via their entity's transform.
3. Each frame, the engine:
   - Updates the Kira listener to match the Audio Listener entity's position and orientation.
   - Updates each spatial track to match its emitter entity's position.
   - Kira automatically calculates distance attenuation and stereo panning.

### Distance Attenuation

| Setting | Effect |
|---------|--------|
| **Min Distance** | Inside this range, the sound plays at full volume with no attenuation. |
| **Max Distance** | Beyond this range, the sound is fully attenuated (silent). |
| Between min and max | Volume decreases according to the rolloff curve. |

### Rolloff Curves

| Curve | Behavior | Best For |
|-------|----------|----------|
| **Logarithmic** | Rapid volume drop near the source, gradual fade at distance. Sounds natural. | Most game sounds — footsteps, gunshots, dialogue |
| **Linear** | Even volume decrease over the entire distance range. | UI-like spatial cues, consistent fade behavior |

### Quick Setup

1. Add an **Audio Listener** to your camera (or player entity).
2. Add an **Audio Player** to the sound source entity.
3. Set a **Clip** path and enable **Spatial**.
4. Adjust **Min Distance** and **Max Distance** to taste.
5. Enter play mode — move the listener entity and hear the sound change.

---

## Effects — Reverb & Delay Sends

The engine has two global effect buses: **Reverb** and **Delay**. Each emitter can send a portion of its signal to these effect buses independently.

### How Sends Work

Audio routing with sends follows a professional mixer pattern:

```
                            ┌──────────────┐
Emitter ──→ Bus Track ──→   │  Main Output │
     │                      └──────────────┘
     ├──→ Reverb Send ──→  (wet reverb signal added to output)
     └──→ Delay Send  ──→  (wet delay signal added to output)
```

- The **dry** signal always plays through the bus at full volume.
- The **send** controls how much additional signal goes to the effect.
- Effect buses use 100% wet mix — the dry signal is already present from the bus.

### Reverb Settings

The built-in reverb uses a Freeverb algorithm:

| Parameter | Value | Description |
|-----------|-------|-------------|
| Feedback | 0.85 | Room size — high value gives a large, resonant space |
| Damping | 0.3 | High-frequency absorption — higher = warmer, less bright |
| Stereo Width | 1.0 | Full stereo reverb field |
| Mix | 1.0 (wet only) | Only the reverb signal is output; dry comes from the bus |

### Delay Settings

The built-in delay is a standard feedback delay:

| Parameter | Value | Description |
|-----------|-------|-------------|
| Delay Time | 375 ms | Tempo-aligned to ~80 BPM quarter note |
| Feedback | -6 dB | Each repeat is half the volume of the previous |
| Mix | 1.0 (wet only) | Only the delayed signal is output |

### Usage Tips

| Reverb Send | Effect |
|-------------|--------|
| 0.0 | No reverb (default) |
| 0.1–0.3 | Subtle room ambience — good for footsteps, UI sounds |
| 0.4–0.6 | Noticeable reverb — good for impacts, environmental sounds |
| 0.7–1.0 | Heavy reverb — caves, cathedrals, dramatic moments |

| Delay Send | Effect |
|------------|--------|
| 0.0 | No delay (default) |
| 0.1–0.2 | Subtle echo — adds depth without being obvious |
| 0.3–0.5 | Noticeable echo — gunshots in open spaces, shouts across valleys |
| 0.6–1.0 | Heavy echo — stylistic effect, rhythmic patterns |

---

## Editor Preview

You can preview any Audio Player's clip without entering play mode.

### How to Preview

1. Select an entity with an Audio Player component.
2. In the inspector, click the **▶** button next to the Clip field.
3. The sound plays immediately on the emitter's configured bus.
4. The preview stops automatically when playback finishes.

### Notes

- Preview plays through the mixer, so mute/solo and bus volumes apply.
- Only one preview can play at a time — starting a new preview stops the previous one.
- Preview is available regardless of play mode state.
- The ▶ button is disabled when the Clip field is empty.

---

## Play Mode & Audio Lifecycle

### What Happens When You Press F5

| Step | Action |
|------|--------|
| 1 | Kira AudioManager is already running (initialized at startup). |
| 2 | Emitters with **Autoplay** enabled are detected. |
| 3 | For each autoplay emitter: load the clip, apply volume/pitch/panning/fade/loop settings, and play on the configured bus. Spatial emitters create spatial tracks. |
| 4 | Scripts begin executing — `on_ready()` fires, then `on_update()` each frame. |
| 5 | Script audio commands (play_sound, play_music, etc.) are queued and processed each frame. |
| 6 | Spatial audio positions are synced every frame from entity transforms. |
| 7 | The mixer syncs UI state to Kira track handles every frame. |

### What Happens When You Stop

| Step | Action |
|------|--------|
| 1 | All active sound handles are stopped immediately. |
| 2 | Music handle is stopped. |
| 3 | All spatial tracks and the listener are dropped. |
| 4 | Per-emitter send tracks are cleared. |
| 5 | Master volume resets to 1.0. |
| 6 | The AudioCommandQueue is drained. |

### Play Mode States and Audio

| State | Audio | How to Enter |
|-------|-------|-------------|
| **Editing** | Only editor preview works | Default state / press Escape during play |
| **Playing** | Full audio — autoplay, scripts, spatial | Press F5 |
| **Paused** | Audio paused but handles kept alive | Press F6 during play |
| **Scripts Only** | Full audio (same as Playing) | Press Shift+F5 |

---

## Scripting API — Sound Effects

Play one-shot or looping sounds from Rhai scripts. Sounds are associated with the calling entity.

### play_sound(path) / play_sound(path, bus)

Play a one-shot sound effect at full volume.

```js
play_sound("assets/sounds/explosion.wav");
play_sound("assets/sounds/explosion.wav", "Sfx");  // explicit bus
```

### play_sound_at_volume(path, volume) / play_sound_at_volume(path, volume, bus)

Play a sound at a specific volume (0.0–2.0).

```js
play_sound_at_volume("assets/sounds/coin.wav", 0.5);
play_sound_at_volume("assets/sounds/coin.wav", 0.5, "Sfx");
```

### play_sound_looping(path, volume) / play_sound_looping(path, volume, bus)

Play a looping sound that repeats indefinitely until stopped.

```js
play_sound_looping("assets/sounds/engine_hum.wav", 0.8);
play_sound_looping("assets/sounds/engine_hum.wav", 0.8, "Ambient");
```

### Notes

- The `path` is relative to the project root.
- When a script calls `play_sound`, the sound is tracked against the **calling entity**. This means `pause_sound()` and `resume_sound()` (without arguments) control that entity's sounds.
- If the entity has an Audio Player component, its pitch/panning/fade/loop settings are applied to the sound automatically.
- If the emitter has non-zero reverb_send or delay_send, the sound is routed through an effects send track.
- The `bus` parameter defaults to `"Sfx"` for all sound functions when not specified.

---

## Scripting API — 3D Spatial Sounds

Play sounds positioned in 3D space. These are automatically attenuated and panned based on the Audio Listener's position.

### play_sound_3d(path, x, y, z) / play_sound_3d(path, x, y, z, bus)

Play a spatial sound at a world position.

```js
// Explosion at world coordinates (10, 0, 5)
play_sound_3d("assets/sounds/explosion.wav", 10.0, 0.0, 5.0);

// On a specific bus
play_sound_3d("assets/sounds/explosion.wav", 10.0, 0.0, 5.0, "Sfx");
```

### play_sound_3d_at_volume(path, volume, x, y, z) / play_sound_3d_at_volume(path, volume, x, y, z, bus)

Play a spatial sound with explicit volume.

```js
play_sound_3d_at_volume("assets/sounds/gunshot.wav", 0.9, 10.0, 0.0, 5.0);
play_sound_3d_at_volume("assets/sounds/gunshot.wav", 0.9, 10.0, 0.0, 5.0, "Sfx");
```

### Notes

- Spatial sounds require an **Audio Listener** in the scene. Without one, the sound still plays but without spatial positioning.
- If the calling entity has an Audio Player with Spatial enabled, the spatial parameters (min/max distance, rolloff) come from the component. Otherwise, defaults are used (min=1.0, max=50.0, logarithmic).
- If the calling entity has a `GlobalTransform`, its position is used instead of the x/y/z arguments.
- Spatial tracks persist per-entity and are updated every frame with the entity's current position.

---

## Scripting API — Music

Music uses streaming playback (loaded progressively from disk, not fully into memory) and loops automatically.

### play_music(path) / play_music(path, bus)

Start playing background music. Replaces any currently playing music.

```js
play_music("assets/music/theme.ogg");
play_music("assets/music/theme.ogg", "Music");
```

### play_music_with_fade(path, volume, fade_in_seconds) / play_music_with_fade(path, volume, fade_in_seconds, bus)

Start music with a fade-in.

```js
play_music_with_fade("assets/music/battle.ogg", 0.8, 2.0);  // 2 second fade-in
play_music_with_fade("assets/music/battle.ogg", 0.8, 2.0, "Music");
```

### stop_music()

Stop music immediately.

```js
stop_music();
```

### stop_music_with_fade(fade_out_seconds)

Stop music with a fade-out.

```js
stop_music_with_fade(2.0);  // 2 second fade-out
```

### crossfade_music(path, volume, duration) / crossfade_music(path, volume, duration, bus)

Crossfade from the current track to a new one. The old track fades out while the new track fades in over the specified duration.

```js
crossfade_music("assets/music/calm.ogg", 1.0, 3.0);  // 3 second crossfade
crossfade_music("assets/music/calm.ogg", 1.0, 3.0, "Music");
```

### Notes

- Only **one** music track plays at a time. Starting new music stops the previous track.
- Music always loops (the loop region is the entire file).
- The `bus` parameter defaults to `"Music"` for all music functions.
- Crossfade is the smoothest way to transition between tracks.

---

## Scripting API — Playback Control

Control sounds that are already playing.

### pause_sound() / pause_sound(entity_id)

Pause the calling entity's sounds, or a specific entity's sounds.

```js
pause_sound();                    // pause my own sounds
pause_sound(other_entity_id);     // pause another entity's sounds
```

When called without arguments on an entity that has no playing sounds, it pauses **all** sounds and music globally.

### resume_sound() / resume_sound(entity_id)

Resume paused sounds.

```js
resume_sound();                   // resume my own sounds
resume_sound(other_entity_id);    // resume another entity's sounds
```

### stop_all_sounds()

Immediately stop all currently playing sounds and music.

```js
stop_all_sounds();
```

### set_master_volume(volume)

Set the global master volume (0.0–1.0). Affects all sounds and music.

```js
set_master_volume(0.5);  // 50% volume
```

---

## Scripting API — Volume & Pitch

Dynamically adjust volume and pitch of playing sounds with optional fade transitions.

### set_sound_volume(volume, fade_seconds) / set_sound_volume(entity_id, volume, fade_seconds)

Smoothly change the volume of an entity's sounds.

```js
// Fade my sounds to 50% over 1 second
set_sound_volume(0.5, 1.0);

// Fade another entity's sounds
set_sound_volume(other_entity_id, 0.3, 0.5);
```

### set_sound_pitch(pitch, fade_seconds) / set_sound_pitch(entity_id, pitch, fade_seconds)

Smoothly change the playback speed/pitch of an entity's sounds.

```js
// Slow down to half speed over 2 seconds
set_sound_pitch(0.5, 2.0);

// Speed up another entity
set_sound_pitch(other_entity_id, 1.5, 0.5);
```

### Notes

- Volume and pitch changes apply to **all** active sound handles on the target entity.
- Set `fade_seconds` to 0 for an instant change.
- Pitch changes affect both speed and pitch together — there is no independent pitch shift.

---

## Scripting API — Querying Sound State

### is_sound_playing(entity_id)

Check if an entity currently has active sounds playing.

```js
if is_sound_playing(self_entity_id) {
    // sound is still playing, don't restart it
} else {
    play_sound("assets/sounds/ambient_loop.wav");
}
```

Returns `true` if the entity has at least one active (non-stopped) sound handle.

---

## Scripting API — Component Properties

Audio Player and Audio Listener properties can be read and written at runtime via the generic property system.

### Audio Player Properties

| Property | Type | Description |
|----------|------|-------------|
| `clip` | String | Audio file path |
| `volume` | Float | Playback volume |
| `pitch` | Float | Playback speed |
| `panning` | Float | Stereo pan (-1.0 to 1.0) |
| `looping` | Bool | Whether the clip loops |
| `loop_start` | Float | Loop start point in seconds |
| `loop_end` | Float | Loop end point in seconds |
| `autoplay` | Bool | Auto-start on play mode |
| `fade_in` | Float | Fade-in duration |
| `bus` | String | Mixer bus name |
| `spatial` | Bool | 3D spatial audio enabled |
| `spatial_min_distance` | Float | Spatial min distance |
| `spatial_max_distance` | Float | Spatial max distance |
| `spatial_rolloff` | String | "Logarithmic" or "Linear" |
| `reverb_send` | Float | Reverb send amount (0.0–1.0) |
| `delay_send` | Float | Delay send amount (0.0–1.0) |

### Audio Listener Properties

| Property | Type | Description |
|----------|------|-------------|
| `active` | Bool | Whether this is the active listener |

### Examples

```js
// Read the emitter's current volume
let vol = get_property(self_entity_id, "volume");

// Change bus at runtime
set_property(self_entity_id, "bus", "Music");

// Enable spatial audio from a script
set_property(self_entity_id, "spatial", true);

// Increase reverb in a cave zone
set_property(self_entity_id, "reverb_send", 0.7);

// Switch active listener to a different entity
set_property(main_camera_id, "active", false);
set_property(cutscene_camera_id, "active", true);
```

---

## Bus Routing

Every sound plays through a mixer bus. The bus determines which mixer channel strip controls it.

### Bus Hierarchy

```
Master ──── (controls overall output)
  ├── SFX ──── (sound effects)
  ├── Music ── (background music)
  ├── Ambient ─ (environmental sounds)
  └── Custom ── (user-created buses)
```

### How Bus Selection Works

| Source | Bus Determined By |
|--------|-------------------|
| Audio Player (autoplay) | The emitter's **Bus** dropdown setting |
| `play_sound()` (no bus argument) | Defaults to `"Sfx"` |
| `play_sound("path", "Ambient")` | Explicit bus argument |
| `play_music()` (no bus argument) | Defaults to `"Music"` |
| Audio Player with script | Emitter's Bus setting is used when the entity has an Audio Player component |

### Creating Custom Buses

1. Open the **Mixer** panel.
2. Click **+ Bus** on the right.
3. Type a name (e.g. "Dialogue") and click **Create**.
4. Reference the bus by name in scripts: `play_sound("clip.wav", "Dialogue")`.
5. Set the bus in the Audio Player inspector dropdown — custom buses appear automatically.

---

## Supported Formats

| Format | Extension | Notes |
|--------|-----------|-------|
| WAV | `.wav` | Uncompressed. Best quality, largest files. Good for short SFX. |
| OGG Vorbis | `.ogg` | Compressed. Good quality, small files. Recommended for music and long clips. |
| MP3 | `.mp3` | Compressed. Universal compatibility. Slight quality loss. |
| FLAC | `.flac` | Lossless compression. Perfect quality, moderate file size. |

### Format Recommendations

| Use Case | Recommended Format |
|----------|-------------------|
| Short sound effects (< 5 seconds) | WAV |
| Long sound effects (> 5 seconds) | OGG |
| Background music | OGG or FLAC |
| Voice/dialogue | OGG |
| Ambient loops | OGG |

---

## Recipes

### Ambient sound on a torch

1. Add an Audio Player to the torch entity.
2. Set Clip to `assets/sounds/fire_crackle.ogg`.
3. Enable **Looping** and **Autoplay**.
4. Enable **Spatial**, set Min Distance=1, Max Distance=15.
5. Add Reverb Send of 0.2 for atmosphere.
6. Enter play mode — the fire sound plays at the torch's position and fades with distance.

### Footsteps from a script

```js
fn props() {
    #{
        footstep_timer: #{ default: 0.0 },
        footstep_interval: #{ default: 0.4, min: 0.1, max: 1.0 }
    }
}

fn on_update() {
    let speed = sqrt(input_x * input_x + input_y * input_y);

    if speed > 0.1 {
        footstep_timer += delta;
        if footstep_timer >= footstep_interval {
            footstep_timer = 0.0;
            play_sound_at_volume("assets/sounds/footstep.wav", 0.6);
        }
    } else {
        footstep_timer = 0.0;
    }
}
```

### Background music with crossfade on zone change

```js
fn props() {
    #{
        current_zone: #{ default: "explore" }
    }
}

fn on_ready() {
    play_music_with_fade("assets/music/explore.ogg", 0.6, 3.0);
}

fn on_update() {
    // Assume zone detection logic sets new_zone
    let new_zone = get_property(self_entity_id, "current_zone");

    if new_zone == "combat" && current_zone != "combat" {
        crossfade_music("assets/music/battle.ogg", 0.8, 2.0);
    }
    if new_zone == "explore" && current_zone != "explore" {
        crossfade_music("assets/music/explore.ogg", 0.6, 2.0);
    }

    current_zone = new_zone;
}
```

### Spatial gunshot from another entity

```js
fn on_update() {
    if is_key_just_pressed("Space") {
        // Play at the enemy's position
        let pos = get_position(enemy_id);
        play_sound_3d_at_volume("assets/sounds/gunshot.wav", 0.9, pos.x, pos.y, pos.z);
    }
}
```

### Duck music during dialogue

```js
fn play_dialogue(clip) {
    // Lower music volume
    set_sound_volume(0.2, 0.5);  // on the music entity

    play_sound(clip);
}

fn on_update() {
    // Check if dialogue finished
    if !is_sound_playing(self_entity_id) && dialogue_playing {
        // Restore music volume
        set_sound_volume(1.0, 0.5);
        dialogue_playing = false;
    }
}
```

### Prevent sound spam with is_sound_playing

```js
fn on_update() {
    if is_key_just_pressed("E") {
        if !is_sound_playing(self_entity_id) {
            play_sound("assets/sounds/interact.wav");
        }
    }
}
```

### Engine sound with pitch tied to speed

```js
fn on_update() {
    let speed = get_velocity_magnitude(self_entity_id);
    let pitch = clamp(0.5 + speed * 0.1, 0.5, 2.0);
    set_sound_pitch(pitch, 0.1);  // smooth 100ms transition
}
```

---

## Architecture Overview

For advanced users — how the audio system is structured internally.

### System Flow Per Frame

```
1. PreScript
   └── autoplay_audio_emitters  (plays autoplay emitters on play mode enter)

2. Script Execution
   └── Rhai scripts push RhaiCommands into thread-local buffer
   └── Commands are drained and routed to AudioCommandQueue

3. CommandProcessing
   ├── process_kira_commands     (converts AudioCommands → Kira API calls)
   └── sync_spatial_audio        (updates listener + emitter positions)

4. Update
   ├── sync_mixer_to_kira        (applies mixer UI state to Kira tracks)
   ├── preview_audio_system      (auto-stop finished previews)
   ├── update_vu_meters          (estimate bus activity for VU display)
   └── sync_project_path         (keeps asset path resolution current)

5. Cleanup
   ├── prune_finished_sounds     (remove stopped handles)
   └── cleanup_kira_on_stop      (full cleanup on play mode exit)

6. Startup (once)
   └── load_mixer_volumes        (restore persisted volumes from config)
```

### Key Types

| Type | Location | Purpose |
|------|----------|---------|
| `KiraAudioManager` | `src/audio/manager.rs` | NonSend resource wrapping Kira's AudioManager, all track handles, spatial state |
| `MixerState` | `src/audio/mixer.rs` | Resource storing per-bus volume/pan/mute/solo/peak state |
| `AudioPreviewState` | `src/audio/preview.rs` | Resource tracking editor preview playback |
| `AudioEmitterData` | `src/component_system/components/audio_emitter.rs` | ECS component with all emitter settings |
| `AudioListenerData` | `src/component_system/components/audio_listener.rs` | ECS component marking the spatial listener |
| `AudioCommandQueue` | `src/scripting/resources/audio.rs` | Resource queue bridging scripts → audio systems |
| `RhaiCommand` | `src/scripting/rhai_commands.rs` | Enum of all scriptable commands (audio subset) |

### Audio Command Pipeline

```
Rhai Script
  → register_fn closure pushes RhaiCommand to thread-local buffer
  → drain_commands() collects them after script execution
  → runtime.rs maps RhaiCommand → AudioCommand in AudioCommandQueue
  → process_kira_commands() drains AudioCommandQueue and calls Kira API
```

---

## Best Practices

### General

- **Use OGG for anything longer than 5 seconds.** WAV is fine for short SFX but wastes memory for longer clips.
- **Set appropriate bus routing.** Don't put everything on "Sfx" — use "Music" for music, "Ambient" for ambient, and create custom buses for dialogue or vehicle sounds.
- **Use Autoplay for ambient sounds.** Rather than scripting `play_sound` in `on_ready`, set Autoplay on the Audio Player. It's cleaner and visible in the inspector.
- **Use spatial audio for world sounds.** If a sound comes from a place in the world, enable Spatial. It adds immersion with minimal effort.
- **Guard against sound spam.** Check `is_sound_playing()` before playing repeated sounds to prevent overlapping audio.

### Spatial Audio

- **Set reasonable min/max distances.** Min=1, Max=50 is a good starting point for most sounds. Adjust based on the sound's loudness and importance.
- **Use Logarithmic rolloff** unless you have a specific reason for Linear. It sounds more natural.
- **Only one active Audio Listener.** Multiple active listeners cause undefined behavior — the first one found each frame wins.
- **Attach the listener to the right entity.** In third-person games, the player character (not the orbiting camera) usually gives better spatial results.

### Performance

- **Kira handles many simultaneous sounds efficiently**, but hundreds of concurrent sounds will still impact performance.
- **Spatial tracks are created per-entity.** Each spatial emitter creates a Kira spatial sub-track. These are reused across multiple sounds on the same entity.
- **Send tracks are created per-entity** when reverb/delay is used. They persist until play mode exits.
- **Music uses streaming playback** — it doesn't load the entire file into memory, so even large music files are efficient.
- **Finished sounds are pruned automatically** each frame. You don't need to manage handle cleanup.

### Mixing

- **Mix at 0 dB (unity).** Set your faders to the 0 dB mark and adjust individual sound volumes rather than pushing the bus volume up.
- **Leave headroom on Master.** If you're pushing Master above 0 dB, your sounds are too quiet individually.
- **Use Solo for debugging.** When something sounds wrong, Solo individual buses to isolate the problem.
- **Reverb/Delay Send values of 0.1–0.3** add depth without being obvious. Higher values are stylistic choices.

---

## Troubleshooting

### "Sound doesn't play"

- **Check the Clip path.** It must be relative to the project root and the file must exist. Look for warnings in the console like `[KiraAudio] Failed to load`.
- **Check the bus.** Is the bus muted? Is another bus soloed? Open the Mixer panel to verify.
- **Are you in play mode?** Scripted sounds only work during play mode. Use the inspector ▶ button for editor previews.
- **Check master volume.** `set_master_volume(0.0)` silences everything.

### "Spatial sound doesn't move"

- **Is there an Audio Listener in the scene?** Spatial audio requires a listener. Add one to your camera or player.
- **Is the listener Active?** Check the Audio Listener inspector — `Active` must be checked.
- **Is Spatial enabled on the emitter?** The emitter's Spatial checkbox must be on.
- **Is the distance too large?** If Max Distance is 10 and you're 50 units away, the sound is fully attenuated. Increase Max Distance.

### "Sound plays but has no reverb"

- **Check Reverb Send.** It must be > 0 on the Audio Player. A value of 0.3–0.5 should be clearly audible.
- **Is the sound playing on a bus?** Reverb sends only work for sounds routed through bus tracks (all normal playback).

### "Music cuts off when I play a new track"

- This is by design. Only one music track plays at a time. Use `crossfade_music()` for smooth transitions instead of `play_music()`.

### "Mixer volumes reset"

- Volumes are persisted to `~/.config/bevy_editor/config.toml`. If this file is deleted or unwritable, volumes reset to defaults.
- Play-mode master volume (set via `set_master_volume()`) is reset when exiting play mode. Mixer panel volumes are not.

### "Sound is distorted or clipping"

- **Reduce volume.** If Volume is above 1.0 on the emitter, or the bus fader is above 0 dB, the signal may clip.
- **Check overlapping sounds.** Many simultaneous sounds on the same bus can sum to a level above clipping. Reduce individual volumes.
- **Check the VU meter.** If the meter is hitting the red zone, reduce the bus volume.

### "Preview works but autoplay doesn't"

- **Is Autoplay enabled?** Check the Audio Player inspector.
- **Is the Clip field set?** Autoplay requires a non-empty clip path.
- **Are you entering play mode?** Autoplay triggers on the transition from editing to play mode (F5). It doesn't trigger on subsequent frames.
