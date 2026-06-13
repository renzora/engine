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

When `distance_relative_speed` is on (the default), move and zoom speed scale with the orbit distance, so navigation stays comfortable whether you're framing a whole level or a single vertex.

### Keyboard shortcuts

These are `EditorAction`s with the default bindings below — all rebindable in **Settings → Keybindings**.

| Default key | Action |
|-------------|--------|
| **F** | Focus Selected — frame the selected entity (engages pivot lock) |
| **A** | Frame All — fit every mesh in the scene |
| **End** | Camera to Cursor — move the pivot to the point under the cursor |
| **Home** | Reset Camera (back to default orbit) |
| **L** | Toggle Pivot Lock |
| **Numpad 5** | Toggle Orthographic / Perspective |
| **Numpad 1** / **Ctrl+Numpad 1** | View Front / Back |
| **Numpad 3** / **Ctrl+Numpad 3** | View Right / Left |
| **Numpad 7** / **Ctrl+Numpad 7** | View Top / Bottom |
| **]** / **[** | Camera speed up / down |

### Pivot lock

Pivot lock (`PivotLock`) keeps the orbit centered on whatever you focused. While it's on, zoom becomes a true dolly (it changes `distance` only, leaving `focus` anchored) and Shift+Right pan is suppressed. It engages automatically on **Focus Selected (F)**, **Frame All (A)**, and **Camera to Cursor (End)**, breaks when you fly with WASD, and toggles with **L**.

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
| `distance_relative_speed` | true | Scale move/zoom by distance from focus |

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

Presets are scriptable: a script on the camera calls `goto_camera_preset("name")` to move that camera entity to a stored angle at runtime. See the [Scripting](/docs/r1-alpha5/api/scripting) page.

## Post-process effects on cameras

Renzora does **not** ask you to insert Bevy's effect components (`Bloom`, `DepthOfField`, …) directly. Instead you author an effect's **settings component** (e.g. `BloomSettings`, `AsciiSettings`, or the GI types `RtLighting` / `LumenLighting`), and the engine proxies it onto the active camera through the `EffectRouting` table.

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
- All the custom shader effects run inside a **single unified fullscreen pass** placed between tonemapping and the end of main-pass post-processing. Each effect only runs when its component is present, so an inactive effect costs nothing.

See the [Post-Processing](/docs/r1-alpha5/rendering/post-processing) page for the full effect catalog (~53 effects) and their fields.

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

```rhai
// Rhai
fn on_update() {
    screen_shake(0.3, 0.2);
}
```

> Functions like `set_camera_fov`, `set_camera_position`, `camera_look_at`, `camera_screen_to_world`, and `camera_world_to_screen` do **not** exist. A few camera verbs (`camera_follow`, `set_camera_target`, `zoom`) are defined in the internal `ScriptCommand` enum but have **no** Lua/Rhai binding, so they are not callable from scripts today. See [Lua scripting](/docs/r1-alpha5/scripting/lua) and [Rhai scripting](/docs/r1-alpha5/scripting/rhai) for the full function surface.
