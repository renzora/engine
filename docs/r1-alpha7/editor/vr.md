# VR Preview

Renzora can play your scene straight into a **VR headset** over OpenXR,
**in-process**: pick **VR Headset** from the Play button's dropdown and
pressing Play lights the headset up with the live scene — no separate window,
no relaunch. The editor keeps running (the viewport panel plays the scene as
usual), scripts and physics run through the normal play mode, and Stop drops
the headset session and returns to editing.

## Requirements

- An OpenXR runtime **active when the editor starts**: Meta Quest Link /
  Air Link (headset in Link mode) or SteamVR, with the matching runtime set
  as the system's default OpenXR runtime. The editor binds to the headset's
  graphics device at boot — connect first, then launch the editor. (Started
  the editor without the headset? Restart it after connecting.)
- A desktop build of the engine (VR is not available on web/mobile exports).

## Playing in VR

1. Connect the headset (Quest: enable Quest Link so you're in the Link home
   environment), then start the editor. The console logs
   `OpenXR runtime detected — booting XR-capable editor` when it worked.
2. Open the Play button's dropdown (the caret next to Play) and choose
   **VR Headset**. The choice persists per-user, like the other targets.
3. Press **Play**. The headset session starts with the live scene; put it on.
4. Press **Stop** to end the headset session and return to flat editing.

Shipped games run in VR with the same engine binary and a flag:

```
renzora.exe --no-editor --project path/to/project --vr
```

(`--vr` game mode auto-starts the session and adds a head-tracked desktop
mirror window for spectators; in the editor, the viewport panel is the
mirror.)

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
