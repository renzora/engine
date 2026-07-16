# VR Preview

Renzora can play your scene straight into a **VR headset** over OpenXR. It's a
play *target*, not a separate editor: pick **VR Headset** from the Play
button's dropdown and pressing Play launches the runtime into your headset —
head tracking, controllers, and locomotion included — while the editor waits,
exactly like the "Window" target.

## Requirements

- An OpenXR runtime installed and active (Meta Quest Link / SteamVR / any
  conformant desktop runtime) with a connected headset.
- A desktop build of the engine (VR is not available on web/mobile exports).

## Playing in VR

1. Open the Play button's dropdown (the caret next to Play) and choose
   **VR Headset**. The choice persists per-user, like the other targets.
2. Press **Play**. The engine saves the scene and launches the runtime as a
   separate process with `--vr`. Put on the headset — the scene starts at the
   tracking origin.
3. Press **Stop** in the editor (or close the mirror window / take off the
   headset and quit) to end the session.

The same thing works from a terminal against any project:

```
renzora.exe --no-editor --project path/to/project --vr
```

## In the headset

| Input | Action |
|---|---|
| Left thumbstick | Smooth locomotion (head-relative) |
| Right thumbstick ← / → | Snap turn (45°, configurable; smooth turn available) |
| Triggers / grips / A B X Y | Exposed to gameplay via the `VrInput` state (script bindings planned) |

- Both hands show **controller wands** (blue = left, orange = right).
- The desktop window shows a **head-tracked mirror** of what the player sees,
  for spectators and debugging.
- The scene's authored game camera is suspended in VR — the headset is the
  camera. Render-to-texture cameras keep working.
- Scene content needs no VR-specific setup: meshes, lights, particles, physics
  and **gaussian splat clouds** all render through the stereo eye cameras
  automatically.

## Notes & current limits

- Controller bindings ship for the Oculus Touch profile; other controllers
  work through your OpenXR runtime's rebinding layer (SteamVR Input, etc.).
- Hand tracking and passthrough are negotiated with the runtime but not yet
  surfaced as gameplay features.
- Grabbing/teleport interactions, VR script APIs, and an in-headset editing
  mode are planned follow-ups.
- If no OpenXR runtime is present, the launch logs an error to the console
  and exits — check that your headset software is running.
