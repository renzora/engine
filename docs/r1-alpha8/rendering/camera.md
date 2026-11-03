# Camera System

How the editor's orbit viewport works, how cameras are spawned at runtime, and how post-process effects attach to them.

Renzora has two distinct camera stories. The **editor viewport** is driven by an orbit controller in the `renzora_camera` crate (editor-only). A **shipped game** has no orbit controller at all — its cameras are ordinary Bevy `Camera3d` entities that come out of the scene. Both share the same post-process routing.

> There is no `FirstPersonController`, `ThirdPersonController`, or `OrbitController` runtime component, and no `CameraShake` component. Those were never part of the engine. The orbit controller below is editor navigation, not a gameplay camera rig.

## The editor orbit camera

The editor viewport camera lives in `renzora_camera::CameraPlugin`, registered as:

```rust
renzora::add!(CameraPlugin, Editor);
```

Because the scope is `Editor`, the controller is installed **only** when the editor bundle is loaded — it never runs in an exported game or on a `--server` host.

### Orbit state

All navigation is stored in one struct that doubles as a `Component` (on the scene camera, so it persists into the scene RON) and a singleton `Resource`:

```rust
pub struct OrbitCameraState {
    pub focus: Vec3,            // the point the camera orbits around
    pub distance: f32,         // distance from the focus
    pub yaw: f32,              // horizontal angle (radians)
    pub pitch: f32,            // vertical angle (radians), clamped to ±1.5
    pub projection_mode: ProjectionMode,
}
```

Defaults are `focus = Vec3::ZERO`, `distance = 4.5`, `yaw = 0.3`, `pitch = 0.4`, `Perspective`. The component is editor-only and is **stripped on export**, so it never ships in a game scene.

### Navigation controls

| Input | Action |
|-------|--------|
| **Right-click + drag** | Look around (yaw/pitch); the pivot is preserved |
| **Right-click + WASD** | Fly — W/S forward/back, A/D strafe, E up, Q down |
| **Middle-click drag** *or* **Alt + Left-click drag** | Orbit around the focus point |
| **Shift + Right-click drag** | Pan — slides the focus in the view plane (suppressed when pivot lock is on) |
| **Scroll wheel** | Dolly zoom (changes `distance` when pivot-locked, otherwise moves the focus along the view ray) |
| **Ctrl (held)** | Move/look at 0.25× speed for fine control |

> In **Edit** (mesh-edit) mode, E/Q are surrendered to the extrude tool, so vertical fly is unavailable — use scroll-dolly or Shift+Right pan instead.

When `distance_relative_speed` is on (the default), move and zoom speed scale with distance, so navigation stays comfortable whether you're framing a whole level or a single vertex. Zoom and orbit scale with the orbit distance. Flying with right mouse and WASD scales with the nearest surface below or ahead of the camera, so it slows as you come down to the ground or up to a wall; over empty space it falls back to the orbit distance.

### Keyboard shortcuts

These are `EditorAction`s with the default bindings below — all rebindable in **Settings → Keybindings**.

| Default key | Action |
|-------------|--------|
| **F** | Focus Selected — frame the selected entity (engages pivot lock) |
| **A** | Frame All — fit every mesh in the scene |
| **End** | Camera to Cursor — move the pivot to the surface under the cursor (the ground plane where there is none) |
| **Home** | Reset Camera (back to default orbit) |
| **L** | Toggle Pivot Lock |
| **Numpad 5** | Toggle Orthographic / Perspective |
| **Numpad 1** / **Ctrl+Numpad 1** | View Front / Back |
| **Numpad 3** / **Ctrl+Numpad 3** | View Right / Left |
| **Numpad 7** / **Ctrl+Numpad 7** | View Top / Bottom |
| **]** / **[** | Camera speed up / down |

### Pivot lock

Pivot lock (`PivotLock`) keeps the orbit centered on whatever you focused. While it's on, zoom becomes a true dolly (it changes `distance` only, leaving `focus` anchored) and Shift+Right pan is suppressed. It engages automatically on **Focus Selected (F)**, **Frame All (A)**, and **Camera to Cursor (End)**, breaks when you fly with WASD, and toggles with **L**.

### Pivot marker

While you orbit, pan or zoom, and for a moment after **F**, **End**, **A** or **Home** moves it, the pivot shows as a small ring with its distance beside it ("128 m", "1.1 km"). Orbit and zoom speed follow that distance, so the label explains a zoom that lurches. The ring is white, or amber while pivot lock is on, and dims when a surface stands between it and the camera (a pivot left underground reads as one). Flying and mouse-look carry the pivot along with the camera, so they do not show it. Turn it off with **Gizmos → Camera Pivot** in the viewport toolbar or **Settings → Viewport → Camera Pivot** (`ViewportSettings::show_pivot`).

### Camera settings

The controller reads tuning from the `CameraSettings` resource, which the viewport header keeps in sync:

| Field | Default | Meaning |
|-------|---------|---------|
| `move_speed` | 10.0 | WASD fly speed |
| `look_sensitivity` | 0.3 | Right-drag look speed |
| `orbit_sensitivity` | 0.5 | Middle/Alt-drag orbit speed |
| `pan_sensitivity` | 1.0 | Pan speed |
| `zoom_sensitivity` | 1.0 | Scroll dolly speed |
| `invert_y` | false | Invert vertical look |
| `distance_relative_speed` | true | Scale move/zoom by distance: flying by the nearest surface, zoom by distance from focus |

## Projection

`ProjectionMode` is `Perspective` or `Orthographic`, toggled with **Numpad 5** or the viewport header. The controller writes a standard Bevy `Projection`:

- **Perspective** — `PerspectiveProjection` with `fov = π/4` (45°) and a very distant far plane (`100_000.0`).
- **Orthographic** — an `OrthographicProjection` using `ScalingMode::FixedVertical`, with the visible world-height matched to the perspective FOV *at the current orbit distance*. This makes the perspective↔ortho toggle seamless and keeps ortho in metre units (not Bevy's default pixel-units scaling).

## Multiple viewports

The editor supports up to four dockable viewport panels. Each viewport has its **own** orbit slot (`focus` / `distance` / `yaw` / `pitch`); the projection mode is shared and driven by the header.

- An `EditorCamera` marker is relocated each frame onto the **focused** viewport's camera, so the single-camera controller, gizmos, and overlays all "just work" on whichever view you're using.
- Non-focused cameras are driven directly from their stored slots, so the views can never converge.
- The relevant markers are `ViewportCamera(usize)` (which slot a camera belongs to), `EditorCamera` (the focused view), `PlayModeCamera` (the in-editor play camera), and `SceneCamera` (any scene camera).

Each viewport camera renders to an **offscreen image** (`RenderTarget::Image`) that the editor displays inside an `renzora_ember` `ImageNode`, rather than drawing straight to the window. The primary viewport camera additionally bakes the shared atmosphere/IBL environment that the other views borrow.

## Runtime (game) cameras

In an exported game the orbit controller is absent. Cameras are plain Bevy `Camera3d` entities authored in the scene and tagged with engine markers:

- `SceneCamera` — marks any camera that belongs to the scene.
- `DefaultCamera` — marks the one that should be active by default.

The runtime picks the active camera as the first `SceneCamera` that is both `DefaultCamera` and `is_active`, falling back to the first active `SceneCamera`.

```rust
use bevy::prelude::*;
use renzora::core::{SceneCamera, DefaultCamera};

fn spawn_game_camera(mut commands: Commands) {
    commands.spawn((
        Name::new("Main Camera"),
        Camera3d::default(),
        Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        SceneCamera,
        DefaultCamera,
    ));
}
```

Render-to-texture and multi-camera setups use Bevy's stock components. Set draw order with `Camera { order, .. }`, and point a camera at a texture with `RenderTarget::Image(handle.into())` (from `bevy::camera`). For split-screen, set each camera's `Camera.viewport` (a `Viewport` whose `physical_position` / `physical_size` are `UVec2` pixel values — not normalized fractions).

## Snap to Viewport & camera presets

Authoring camera angles in the editor is two clicks:

- **Snap to Viewport** — right-click a camera in the hierarchy (or use the button on its **Camera** component) to move that camera so it sits exactly where the editor fly-camera is looking. Parent-aware: a camera under a rig/empty lands at the right world pose.
- **Camera Presets** — the camera's inspector has a *Camera Presets* section. *Capture current view* saves the current editor view as a named angle. Each preset row has three actions: **Go to** drives the editor fly-camera to that angle (preview it in the viewport), **Snap to Viewport** overwrites the preset with the current editor view, and the trash icon deletes it. Presets live in a `CameraPresets` component (`Vec<CameraPreset { name, translation, rotation }>`, world-space) that serializes into the scene.

Presets are scriptable: a script on the camera calls `goto_camera_preset("name")` to move that camera entity to a stored angle at runtime. See the [Scripting](/docs/r1-alpha8/api/scripting) page.

## Post-process effects on cameras

Renzora does **not** ask you to insert Bevy's effect components (`Bloom`, `DepthOfField`, …) directly. Instead you author an effect's **settings component** (e.g. `BloomSettings`, `AsciiSettings`, or Solari's `SolariGi`), and the engine proxies it onto the active camera through the `EffectRouting` table.

```rust
use bevy::prelude::*;
use renzora_bloom_effect::BloomSettings;

fn enable_bloom(mut commands: Commands, camera: Single<Entity, With<Camera3d>>) {
    commands.entity(*camera).insert(BloomSettings {
        intensity: 0.15,
        threshold: 0.5,
        enabled: true,
        ..default()
    });
}
```

How routing works:

- **At runtime**, the active camera is the resolved `SceneCamera`, and the sources are that camera entity plus every non-camera entity. So a `*Settings` component placed on the camera **or** on any other entity (a "World Environment" entity, say) is applied to the active camera.
- **In the editor**, the same settings are routed to every viewport camera while editing, and to the play camera during play mode.
- For Bevy built-in effects, a per-effect sync system inserts/removes the stock component (`BloomSettings` → `Bloom`, and similarly for DoF, SSAO, SSR, tonemapping, fog, atmosphere, skybox, etc.).
- A camera with an enabled atmosphere routed to it clears to **black**, because the sky is drawn over the camera's clear colour and whatever is behind it shows through where the air scatters little, which at night is almost everywhere. Left on the default grey, a night sky rendered as a flat grey overcast. Only a camera on `ClearColorConfig::Default` is changed; a clear colour you set yourself is kept, and turning the sky off gives the default back.
- All the custom shader effects run inside a **single unified fullscreen pass** placed between tonemapping and the end of main-pass post-processing. Each effect only runs when its component is present, so an inactive effect costs nothing.

See [Post-Processing Effects](../extending/post-processing.md) for how the 53 shipped effects are authored, and [Render Pipeline](./pipeline.md) for how they compose.

## Anti-aliasing

Anti-aliasing is set in the camera's **Camera** section, on two rows: **Anti-Aliasing** (**Off**, or one of the methods below) and **AA Quality**, whose choices follow the method and which is hidden while anti-aliasing is Off. New scene cameras start on TAA. One method runs at a time: they all attack the same jagged edges, and two at once only blur twice (TAA under FXAA) or fight over one camera jitter (TAA and DLSS).

The setting is stored in its own component, `renzora_antialiasing::AntiAliasing`; the Camera section only draws it. Off switches the component's `enabled` off rather than removing it, so the method and each method's quality are still there when it is turned back on. A camera without the component reads as Off, and choosing a method adds one.

| Method | Quality | What it is |
|---|---|---|
| **MSAA** | 2x, 4x, 8x | Several samples per pixel at triangle edges. Sharp and stable, but smooths only geometry edges, and only works where the render path allows it (below). |
| **FXAA** | Low to Extreme | A cheap full-screen pass that blurs along detected edges. Softens the whole image a little. |
| **SMAA** | Low to Ultra | Edge detection like FXAA with a sharper result. Does not fix shimmer in motion. |
| **TAA** | (none) | Jitters the camera and blends frames. The best on thin detail and shimmer (foliage, specular, soft shadows), and what most of the rendering is tuned for. Can ghost on fast motion. |
| **DLSS** | Auto, DLAA, Quality, Balanced, Performance, Ultra Performance | NVIDIA DLSS Super Resolution: upscaling and anti-aliasing in one. See [DLSS](./dlss.md#dlss-without-solari). |

Sharpening (**Sharpening (CAS)**) is a separate component that sits on top of any method, mostly to give back the detail TAA and DLSS soften.

### When a method cannot run

Not every method works on every camera. The **Anti-Aliasing** dropdown puts an amber warning icon on each option that cannot run on this camera right now, and beside the box while that option is the selected one; hover it for the reason and what to change.

| Method | Cannot run when |
|---|---|
| MSAA | the camera renders **Deferred** (Project Settings > Rendering, or Solari forcing it), or has **SSAO**, the **sky** (atmosphere) or **order-independent transparency** on |
| TAA | the viewport's Graphics Quality is **Low**, or Solari's DLSS Ray Reconstruction is anti-aliasing the camera already |
| DLSS | the build has no DLSS SDK, the GPU or renderer cannot run it (an NVIDIA RTX GPU on Vulkan), or Ray Reconstruction is already on the camera |
| FXAA, SMAA | always run |

A method that cannot run is held off, not forced: the camera renders with no anti-aliasing until whatever is in the way goes, and then the method takes effect by itself. The choice stays on the component. Several of these combinations crash the renderer rather than failing quietly, which is why the engine refuses them.

What decides it is the cameras that actually render: in the editor, the viewport cameras the scene camera's settings are routed onto. The rules live in one place (`renzora_antialiasing::compat`), read by both the inspector's warnings and the system that applies the method, so the warning and the viewport always agree.

Scenes saved with the old per-method components (`TaaSettings`, `FxaaSettings`, `SmaaSettings`) still load: each camera's old components are folded into one Anti-Aliasing as the scene loads, keeping the method that was on (TAA first, then SMAA, then FXAA) and each method's quality.

## Tonemapping

The camera's tonemapping curve is the **Tonemapping** row of its **Camera** section: **None** (no curve), Reinhard, Reinhard Luminance, ACES Fitted, AgX, Somewhat Boring, Tony McMapface (the default) or Blender Filmic. It is stored in `renzora_tonemapping::TonemappingSettings`, which the section only draws; exposure is the section's **EV100**, not part of tonemapping.

Older scenes that carry `TonemappingSettings` on a non-camera entity (the World Environment) still show it there as its own Tonemapping section.

## Auto Exposure (keeping night dark)

`AutoExposureSettings` wraps Bevy's histogram auto-exposure: it meters the scene and animates the camera's exposure so the average pixel lands on middle gray (eye adaptation). Because it *always* targets middle gray, a genuinely dark night scene gets **brightened** — physically faithful, but usually not the night look you want.

Two extra knobs shape the response without touching daytime, via Bevy's exposure-compensation curve:

| Field | Meaning |
|---|---|
| **Keep Night Dark** (`keep_dark_strength`, 0–1) | How much dark scenes resist being lifted to middle gray. `0` = vanilla Bevy AE (night washes out); `1` ≈ the metered darkness is preserved (night stays night). Default `0.7`. |
| **Keep-Dark Pivot (EV)** (`keep_dark_pivot_ev`) | The metered brightness (EV-100) at/above which **no** compensation is applied, so daytime is left exactly as before. Compensation ramps in below it. Default `2.0`. Raise it if nights still wash out; lower it if dusk/interiors get too dark. |

The curve is flat (zero) from the pivot upward and ramps negative below it, so the day exposure you already like is unchanged while night is pulled back down. Both knobs are live — tune them in the inspector (Auto Exposure section) without rebuilding.

## Camera shake from scripts

The one camera-facing script function is `screen_shake(intensity, duration)`. It is registered in **both** backends:

```lua
-- Lua
function on_update()
    if is_colliding then
        screen_shake(0.3, 0.2)  -- intensity, duration (seconds)
    end
end
```

> Functions like `set_camera_fov`, `set_camera_position`, `camera_look_at`, `camera_screen_to_world`, and `camera_world_to_screen` do **not** exist. A few camera verbs (`camera_follow`, `set_camera_target`, `zoom`) are defined in the internal `ScriptCommand` enum but have **no** Lua binding, so they are not callable from scripts today. See [Lua scripting](/docs/r1-alpha8/scripting/lua) for the full function surface.
