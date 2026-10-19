# VR

A Renzora game can run in a **VR headset** over OpenXR. The editor no longer plays into a headset itself: Play always runs the game as its own program (see [Playing Your Game](/docs/r1-alpha8/editor/play-mode)), so to try a scene in VR, run the game with the `--vr` flag:

```
renzora.exe --no-editor --project path/to/project --vr
```

`--vr` starts the headset session as soon as the game boots and adds a head-tracked desktop mirror window for spectators.

## Requirements

- An OpenXR runtime **active when the game starts**: Meta Quest Link / Air Link (headset in Link mode) or SteamVR, with the matching runtime set as the system's default OpenXR runtime. Connect the headset first, then launch.
- A desktop build of the engine (VR is not available on web/mobile exports).

## In the headset

| Input | Action |
|---|---|
| Left thumbstick | Smooth locomotion (head-relative) |
| Right thumbstick ← / → | Snap turn (45°, configurable; smooth turn available) |
| Triggers / grips / A B X Y | Exposed to gameplay via the `VrInput` state (script bindings planned) |

- Both hands show **controller wands** (blue = left, orange = right).
- The desktop window shows a **head-tracked mirror** of what the player sees, for spectators and debugging.
- The scene's authored game camera is suspended in VR: the headset is the camera. Render-to-texture cameras keep working.
- Scene content needs no VR-specific setup: meshes, lights, particles, physics and **gaussian splat clouds** all render through the stereo eye cameras automatically.

## Notes & current limits

- Controller bindings ship for the Oculus Touch profile; other controllers work through your OpenXR runtime's rebinding layer (SteamVR Input, etc.).
- Hand tracking and passthrough are negotiated with the runtime but not yet surfaced as gameplay features.
- Grabbing/teleport interactions and VR script APIs are planned follow-ups.
- If no OpenXR runtime is present, the launch logs an error and exits; check that your headset software is running.
