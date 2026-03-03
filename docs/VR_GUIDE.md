# VR Guide

A complete guide to the Renzora Engine VR/XR system — headset setup, controller input, hand tracking, VR components, mixed reality, editor panels, and scripting integration.

The engine uses **OpenXR** via `bevy_mod_openxr` as its VR backend, providing cross-headset support for SteamVR, Meta Quest (via Link), Valve Index, HTC Vive, and any OpenXR-compliant device. VR is fully integrated with the ECS, the inspector, the scripting system, and the editor panel system.

---

## Table of Contents

1. [How VR Works in the Engine](#how-vr-works-in-the-engine)
2. [Prerequisites](#prerequisites)
3. [Building & Launching](#building--launching)
4. [VR Settings Panel](#vr-settings-panel)
5. [Controller Input](#controller-input)
6. [Hand Tracking](#hand-tracking)
7. [Locomotion](#locomotion)
8. [VR Components](#vr-components)
9. [Mixed Reality & Passthrough](#mixed-reality--passthrough)
10. [Editor Panels](#editor-panels)
11. [VR Development Layout](#vr-development-layout)
12. [Scripting API — VR State](#scripting-api--vr-state)
13. [Scripting API — Head Tracking](#scripting-api--head-tracking)
14. [Scripting API — Controller Input](#scripting-api--controller-input)
15. [Scripting API — Hand Tracking](#scripting-api--hand-tracking)
16. [Scripting API — VR Commands](#scripting-api--vr-commands)
17. [Scripting API — Component Properties](#scripting-api--component-properties)
18. [VR Configuration Reference](#vr-configuration-reference)
19. [Session Lifecycle](#session-lifecycle)
20. [Recipes](#recipes)
21. [Architecture Overview](#architecture-overview)
22. [Best Practices](#best-practices)
23. [Troubleshooting](#troubleshooting)

---

## How VR Works in the Engine

VR in Renzora follows an **opt-in activation** pattern:

1. **Build with XR**: Compile with `--features xr` to include OpenXR support. Without this feature, all VR code is excluded and the engine runs as a pure desktop application.
2. **Launch with `--vr`**: Pass the `--vr` CLI flag to start the OpenXR session. This replaces the default Bevy rendering pipeline with stereo rendering, head tracking, and controller input.
3. **Design in the editor**: Add VR components (controllers, teleport areas, grabbables, etc.) to entities in the inspector. These are data components — they describe behavior but don't simulate until play mode.
4. **Play mode**: Press `F5` and put on your headset. All VR systems activate: tracking, input, locomotion, interaction, and spatial audio.
5. **Stop**: Exit play mode and everything resets. VR tracking continues (you can still see the editor in the headset) but gameplay systems stop.

The engine supports simultaneous desktop and VR use — the desktop window acts as a mirror view of what the VR user sees, and the editor is fully functional on the desktop display while VR is active.

---

## Prerequisites

### Hardware

Any OpenXR-compliant VR headset:

| Headset | Connection | Notes |
|---------|-----------|-------|
| Meta Quest 2/3/Pro | Quest Link (USB) or Air Link (Wi-Fi) | Hand tracking supported on Quest Pro/3 |
| Valve Index | SteamVR (USB + DisplayPort) | Index controller finger tracking |
| HTC Vive / Vive Pro | SteamVR (USB + DisplayPort) | Vive Tracker support for full-body |
| Windows Mixed Reality | WMR Portal | Via SteamVR bridge |
| Pico 4 | Streaming Assistant | Via SteamVR |

### Software

- **SteamVR** installed and running (or Meta Quest Link for Quest headsets)
- **cmake** installed and in PATH (required to build the OpenXR native SDK)
  - Windows: `winget install Kitware.CMake` or download from [cmake.org](https://cmake.org)
  - Linux: `sudo apt install cmake`
  - macOS: `brew install cmake`

### Verifying Your Setup

Before building the engine with XR, verify that your headset is connected:

1. Open SteamVR (or Quest Link).
2. Confirm the headset shows as connected (green status).
3. Verify controllers are tracked (visible in the SteamVR status window).

---

## Building & Launching

### Build with XR support

```bash
cargo build --features editor,xr
```

### Run the editor in VR mode

```bash
cargo run --features editor,xr -- --vr
```

### Run the editor without VR (XR compiled but inactive)

```bash
cargo run --features editor,xr
```

VR panels and components are visible in the editor but all tracking returns zeros and VR systems are dormant. Useful for scene setup on a machine without a headset.

### Build a standalone VR game (runtime only)

```bash
cargo build --features xr --bin renzora-runtime
```

---

## VR Settings Panel

Open via the panel menu: **Window > VR > VR Settings**.

This is the main control center for all VR configuration. Changes are persisted per-project.

### Headset Status

Displays the current connection state, headset name, refresh rate, and controller battery levels. Updated in real-time from the OpenXR session.

| Field | Description |
|-------|-------------|
| **Status** | Current session state (Disconnected, Initializing, Ready, Focused, Visible, Stopping, Stopped) |
| **Headset** | Runtime-reported headset name (e.g., "Quest 3", "Valve Index") |
| **Refresh Rate** | Display refresh rate in Hz |
| **Battery** | Left/right controller battery percentage (when available) |

### Rendering

| Setting | Range | Default | Description |
|---------|-------|---------|-------------|
| **Render Scale** | 0.5–2.0 | 1.0 | Resolution multiplier per eye. 0.5 = half resolution (performance), 1.5 = supersampled (quality) |

### Locomotion

| Setting | Options | Default | Description |
|---------|---------|---------|-------------|
| **Mode** | Teleport / Smooth / Both | Teleport | How the player moves in VR |
| **Speed** | 0.5–10.0 m/s | 2.0 | Movement speed for smooth locomotion |
| **Hand** | Left / Right | Left | Which controller's thumbstick controls movement |

### Turning

| Setting | Range | Default | Description |
|---------|-------|---------|-------------|
| **Snap Angle** | 0°–90° | 45° | Degrees per snap turn. 0° = smooth turning |

### Comfort

| Setting | Range | Default | Description |
|---------|-------|---------|-------------|
| **Vignette** | 0.0–1.0 | 0.3 | Tunnel vision intensity during locomotion. Reduces motion sickness. |
| **Seated Mode** | on/off | off | Switches from STAGE (standing) to LOCAL (seated) reference space |

### Input

| Setting | Range | Default | Description |
|---------|-------|---------|-------------|
| **Hand Tracking** | on/off | on | Enable optical hand tracking (Quest Pro/3, Index) |
| **Deadzone** | 0.05–0.5 | 0.2 | Thumbstick dead zone radius. Increase if you get drift. |

### Mixed Reality

| Setting | Options | Default | Description |
|---------|---------|---------|-------------|
| **Passthrough** | on/off | off | Show real-world camera feed through the headset |
| **Blend Mode** | Opaque / Additive / Alpha Blend | Opaque | How virtual content blends with passthrough |
| **Foveated Rendering** | on/off | on | Reduce resolution in peripheral vision (when headset supports it) |

### Capabilities

Read-only indicators showing which features your headset supports:

- Hand Tracking — Supported / Not Available
- Passthrough — Supported / Not Available
- Eye Tracking — Supported / Not Available
- Foveated Rendering — Supported / Not Available
- Reference Space — Stage / Local

---

## Controller Input

The engine supports all standard OpenXR controller inputs with bindings for multiple controller profiles:

### Supported Controllers

| Profile | Controllers |
|---------|------------|
| Oculus Touch | Quest 2/3/Pro controllers |
| Valve Index | Index Knuckles controllers |
| HTC Vive | Vive wand controllers |
| KHR Simple | Fallback for any OpenXR controller |

### Input Mapping

| Input | Type | Description |
|-------|------|-------------|
| **Trigger** | Float (0.0–1.0) | Index finger squeeze. Pressed = above 0.5. |
| **Grip** | Float (0.0–1.0) | Middle finger squeeze. Pressed = above 0.5. |
| **Thumbstick** | Vector (x, y) | Thumbstick/trackpad position. Range -1.0 to 1.0 per axis. |
| **Thumbstick Click** | Bool | Press the thumbstick down |
| **Button A / X** | Bool | Primary face button (A on right, X on left) |
| **Button B / Y** | Bool | Secondary face button (B on right, Y on left) |
| **Menu** | Bool | Menu button (left hand only on most controllers) |

### Poses

Each controller provides two tracked poses:

| Pose | Description |
|------|-------------|
| **Grip** | Position and rotation at the center of the grip (where you hold the controller) |
| **Aim** | Position and rotation of the pointing direction (forward from the controller) |

Both poses provide full 6DOF tracking (position + rotation).

---

## Hand Tracking

When enabled and supported by the headset, hand tracking provides skeleton data for bare hands without controllers.

### Joint Data

Each hand reports 26 tracked joints following the OpenXR hand skeleton model:

| Joint Group | Joints | Count |
|-------------|--------|-------|
| Wrist | Wrist, Palm | 2 |
| Thumb | Metacarpal, Proximal, Distal, Tip | 4 |
| Index | Metacarpal, Proximal, Intermediate, Distal, Tip | 5 |
| Middle | Metacarpal, Proximal, Intermediate, Distal, Tip | 5 |
| Ring | Metacarpal, Proximal, Intermediate, Distal, Tip | 5 |
| Little | Metacarpal, Proximal, Intermediate, Distal, Tip | 5 |

### Gesture Detection

The engine computes two gesture strengths from the raw joint data:

| Gesture | Calculation | Use Case |
|---------|-------------|----------|
| **Pinch Strength** | Distance from thumb tip to index tip. 0–6 cm mapped to 1.0–0.0. | UI interaction, precision selection |
| **Grab Strength** | Average fingertip-to-palm distances (index, middle, ring, little). 0–12 cm mapped to 1.0–0.0. | Grabbing objects, fist gestures |

### Headsets with Hand Tracking

| Headset | Support | Notes |
|---------|---------|-------|
| Meta Quest Pro | Full | Best quality, simultaneous controller + hand tracking |
| Meta Quest 3 | Full | Simultaneous tracking |
| Meta Quest 2 | Basic | Cannot use controllers and hands simultaneously |
| Valve Index | Partial | Finger tracking via Knuckles controllers (not optical) |

---

## Locomotion

Three locomotion modes are available, configurable in VR Settings:

### Teleport

Point the thumbstick forward to project an arc onto surfaces with a **VR Teleport Area** component. Release to teleport. The comfort vignette activates briefly during the teleport.

### Smooth

Push the locomotion hand's thumbstick to move in that direction. Speed is configurable (default 2.0 m/s). The comfort vignette activates proportionally to movement speed.

### Turning

The non-locomotion hand's thumbstick controls rotation:

| Setting | Behavior |
|---------|----------|
| Snap Angle > 0° | Discrete snap turns at the configured angle. Cooldown prevents repeated turns from a held thumbstick. |
| Snap Angle = 0° | Smooth continuous turning at a rate proportional to thumbstick deflection. |

---

## VR Components

Add VR components to entities via the Inspector's **Add Component** button. All VR components appear under the **VR** category.

### VR Controller

Represents a tracked VR controller entity.

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| **Hand** | Dropdown | Right | Left or Right |
| **Show Laser** | Bool | true | Render a laser pointer ray from the controller |
| **Laser Color** | Color | Blue (0.2, 0.6, 1.0, 0.8) | RGBA color of the laser |
| **Laser Length** | Float | 5.0 m | Maximum length of the laser ray |
| **Show Model** | Bool | true | Render a controller mesh model |

### VR Teleport Area

Marks a surface as a valid teleport destination. Attach to floor/ground entities.

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| **Enabled** | Bool | true | Whether this area accepts teleports |
| **Indicator Color** | Color | Green (0.0, 0.8, 0.4, 0.6) | Color of the teleport reticle when aiming at this surface |
| **Restrict to Bounds** | Bool | false | Only allow teleport within the mesh bounds (not the full collider area) |

### VR Grabbable

Makes an entity grabbable by VR controllers. Requires a physics collider for grab detection.

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| **Grab Type** | Dropdown | Offset | **Snap**: object jumps to hand. **Offset**: maintains relative position. **Distance**: force-grab from afar. |
| **Throwable** | Bool | true | Allow the object to be thrown when released |
| **Force Multiplier** | Float | 1.5 | Throw velocity multiplier |
| **Max Grab Distance** | Float | 5.0 m | Maximum range for Distance grab type |
| **Highlight Color** | Color | Cyan (0.4, 0.8, 1.0, 0.3) | Highlight tint when in grab range |

### VR Hand Model

Renders a visual representation of the player's hand or controller.

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| **Hand** | Dropdown | Right | Left or Right |
| **Model Type** | Dropdown | Controller | **Controller**: default controller mesh. **Hand**: skeletal hand mesh. **Custom**: user-specified mesh. |
| **Custom Mesh** | String | (empty) | Asset path for custom model (only when Model Type = Custom) |
| **Visible** | Bool | true | Show/hide the hand model |

### VR Pointer

Casts a ray from the controller's aim pose for UI interaction and object selection.

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| **Hand** | Dropdown | Right | Left or Right |
| **Enabled** | Bool | true | Whether the pointer ray is active |
| **Ray Length** | Float | 10.0 m | Maximum ray distance |
| **Ray Color** | Color | White (1.0, 1.0, 1.0, 0.5) | RGBA color of the ray line |
| **Ray Width** | Float | 0.002 m | Thickness of the ray line |
| **Show Cursor** | Bool | true | Show a dot at the ray hit point |
| **Cursor Size** | Float | 0.02 m | Diameter of the cursor dot |
| **Interact Layers** | Int | all | Collision layer mask for the raycast |

### VR Snap Zone

A magnetic snap point for grabbable objects. When a grabbed object is released near a snap zone, it snaps to the zone's transform.

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| **Snap Radius** | Float | 0.1 m | Distance within which objects snap |
| **Highlight When Near** | Bool | true | Show a highlight when a compatible grabbable is nearby |
| **Highlight Color** | Color | Green (0.2, 1.0, 0.4, 0.4) | Highlight gizmo color |
| **Accepted Tags** | List | (empty) | Only snap grabbables with matching tags. Empty = accept all. |
| **Occupied** | Bool | false | (Read-only) Whether an object is currently snapped here |

Use snap zones for holsters, weapon racks, inventory slots, puzzle piece receptacles, and tool holders.

### VR Climbable

Marks a surface as climbable. When the player grips near this surface, hand movement translates inversely to move the player (climbing motion).

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| **Enabled** | Bool | true | Whether climbing is active on this surface |
| **Grip Distance** | Float | 0.1 m | How close the hand must be to initiate a grab |
| **Surface Normal** | Vec3 | (0, 0, 1) | Outward direction of the climbable surface |

Attach to ladders, walls, cliff faces, or any surface you want the player to climb.

### VR Spatial Anchor

Creates a persistent OpenXR spatial anchor at the entity's world position. Anchors maintain their real-world position even as tracking drifts.

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| **Anchor ID** | String | (empty) | Persistent identifier for the anchor |
| **Persist Across Sessions** | Bool | false | Store the anchor for retrieval in future sessions |
| **Anchor Status** | String | "unanchored" | (Read-only) Current state: unanchored, anchoring, anchored, or lost |

Use spatial anchors for mixed reality experiences where virtual objects must stay locked to real-world positions.

### VR Overlay Panel

A world-space UI panel that can display content and respond to VR pointer input.

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| **Width** | Float | 0.5 m | World-space width of the panel |
| **Pixels Per Meter** | Float | 1000.0 | Resolution of the panel texture |
| **Follow Head** | Bool | false | Billboard — always face the user |
| **Curved** | Bool | false | Apply cylindrical curvature |
| **Curvature Radius** | Float | 1.5 m | Radius of the curve (only when Curved is on) |
| **Interactive** | Bool | true | Respond to VR pointer raycast input |

Use for in-world menus, health bars, status displays, or floating information panels.

### VR Tracked Object

Represents a generic tracked device such as a Vive Tracker for full-body tracking.

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| **Tracker Role** | Dropdown | Waist | Role: left_foot, right_foot, waist, chest, elbow_left, elbow_right, knee_left, knee_right, camera, keyboard, or custom |
| **Serial Number** | String | (empty) | Specific device serial. Empty = any device matching the role. |
| **Tracked** | Bool | false | (Read-only) Whether the device is currently tracked |

### VR Passthrough Window

In mixed reality mode, this component turns a mesh into a portal showing the real world through the VR environment.

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| **Enabled** | Bool | true | Whether the passthrough window is active |
| **Opacity** | Float | 1.0 | Transparency of the passthrough view (0.0–1.0) |
| **Edge Color** | Color | Transparent | Border color around the passthrough region |

Attach to a mesh (plane, sphere, custom shape) — the mesh geometry defines the window shape.

---

## Mixed Reality & Passthrough

Passthrough lets the real-world camera feed show through the headset, enabling mixed reality experiences where virtual objects coexist with the physical environment.

### Enabling Passthrough

1. Open **VR Settings** panel.
2. In the **Mixed Reality** section, check **Passthrough**.
3. Choose a **Blend Mode**:

| Blend Mode | Effect | Best For |
|------------|--------|----------|
| **Opaque** | Full VR — no passthrough visible | Standard VR experiences |
| **Additive** | Virtual content renders as a holographic overlay on the real world | HoloLens-style AR, transparent UI |
| **Alpha Blend** | Virtual content composited over passthrough with proper alpha | Full mixed reality, virtual objects in real rooms |

### Passthrough Windows

For selective passthrough (show the real world only through certain shapes), use the **VR Passthrough Window** component on mesh entities. This creates portal-style windows into the real world within an otherwise fully virtual scene.

### Requirements

- Headset must support passthrough (Quest Pro/3, Vive XR Elite, Pico 4)
- The Capabilities section in VR Settings shows whether passthrough is available

---

## Editor Panels

### VR Input Debug

**Window > VR > VR Input Debug**

Real-time visualization of all VR input state. Essential for debugging controller bindings and hand tracking.

| Section | What It Shows |
|---------|---------------|
| **Controller Status** | Per-hand tracked indicator (green/red), battery level |
| **Buttons & Axes** | All button states with analog bars for trigger/grip (0.0–1.0) |
| **Thumbsticks** | XY plot visualization showing thumbstick position with deadzone circle |
| **Poses** | Live grip and aim position/rotation as formatted Vec3/Quat |
| **Hand Tracking** | Per-hand tracked indicator, pinch and grab strength bars |

### VR Session

**Window > VR > VR Session**

Session lifecycle state and OpenXR diagnostics.

| Section | What It Shows |
|---------|---------------|
| **Session State** | Color-coded status badge (green=Focused, yellow=Visible, red=Disconnected) |
| **Headset Info** | Runtime name, reference space type |
| **Capabilities** | Checkmarks for supported features |
| **Frame Timing** | Predicted display time, display period, should_render flag, target vs actual framerate |
| **Extensions** | Collapsible list of all enabled OpenXR extensions |

### VR Performance

**Window > VR > VR Performance**

VR-specific performance metrics focused on frame budget and reprojection.

| Section | What It Shows |
|---------|---------------|
| **Frame Budget** | Bar showing current frame time vs budget. Green (<80%), Yellow (80–100%), Red (>100% = reprojection) |
| **Frame Time Graph** | Rolling graph with target framerate line |
| **Reprojection** | Count and percentage of frames requiring ASW/ATW/motion smoothing |
| **Resolution** | Current render resolution per-eye, effective render scale, foveation status |

### VR Camera Preview

**Window > VR > VR Camera Preview**

Desktop preview of what the VR user sees.

| Section | What It Shows |
|---------|---------------|
| **View Mode** | Switch between Left Eye, Right Eye, or Center view |
| **Preview Viewport** | Renders the VR camera view as an egui texture |
| **Overlay Toggles** | Grid overlay for distortion checking, tracking bounds visualization |
| **HMD Info** | Resolution per-eye, refresh rate, IPD, FOV |

---

## VR Development Layout

Select **Window > Layout > VR Development** to get a pre-built workspace with all VR panels arranged for development:

```
+------------------+------------------------------------------+------------------+
|                  |                                          |                  |
|    Hierarchy     |               Viewport                   |    Inspector     |
|                  |                                          |                  |
|                  |                                          +------------------+
|                  |                                          |                  |
|                  |                                          | VR Settings      |
|                  |                                          | (tabbed with     |
+------------------+------------------------------------------+  VR Session)     |
|                                                             |                  |
|  Console  |  VR Input Debug  |  VR Performance              |                  |
|                                                             +------------------+
+-------------------------------------------------------------+
```

---

## Scripting API — VR State

Check VR availability and session status from Rhai scripts. All functions return safe defaults when not in VR mode.

### is_vr_active()

Returns `true` if VR mode is active (the `--vr` flag was passed and a session is running).

```js
if is_vr_active() {
    // VR-specific logic
} else {
    // Desktop fallback
}
```

### get_vr_session_status()

Returns the current session status as a string.

```js
let status = get_vr_session_status();
// Returns: "disconnected", "initializing", "ready", "focused",
//          "visible", "stopping", or "stopped"
```

| Status | Meaning |
|--------|---------|
| `"disconnected"` | No headset connected |
| `"initializing"` | OpenXR session starting up |
| `"ready"` | Session ready, waiting for user to put on headset |
| `"focused"` | Headset on, fully rendering |
| `"visible"` | App visible but not focused (e.g., system menu open) |
| `"stopping"` | Session shutting down |
| `"stopped"` | Session ended |

### get_vr_headset_name()

Returns the connected headset's name as reported by the OpenXR runtime.

```js
let headset = get_vr_headset_name();
// e.g., "Quest 3", "Valve Index", "VIVE Pro"
```

---

## Scripting API — Head Tracking

### get_vr_head_position()

Returns the headset position as a map with `x`, `y`, `z` fields (meters, in world space).

```js
let head = get_vr_head_position();
print(`Head at: ${head.x}, ${head.y}, ${head.z}`);
```

### get_vr_head_rotation()

Returns the headset orientation as a quaternion map with `x`, `y`, `z`, `w` fields.

```js
let rot = get_vr_head_rotation();
// rot.x, rot.y, rot.z, rot.w
```

---

## Scripting API — Controller Input

All controller functions take a hand parameter: `"left"` or `"right"`.

### get_vr_controller_position(hand) / get_vr_controller_rotation(hand)

Returns the grip pose position or rotation for the specified hand.

```js
let pos = get_vr_controller_position("right");  // #{x, y, z}
let rot = get_vr_controller_rotation("right");  // #{x, y, z, w}
```

### get_vr_trigger(hand) / is_vr_trigger_pressed(hand)

Trigger value (0.0–1.0) and pressed state (above 0.5 threshold).

```js
let trigger_amount = get_vr_trigger("right");  // 0.0 to 1.0

if is_vr_trigger_pressed("right") {
    // Trigger is more than half pulled
}
```

### get_vr_grip(hand) / is_vr_grip_pressed(hand)

Grip value (0.0–1.0) and pressed state (above 0.5 threshold).

```js
let grip = get_vr_grip("left");

if is_vr_grip_pressed("left") {
    // Gripping
}
```

### get_vr_thumbstick(hand)

Thumbstick position as a map with `x` and `y` fields (-1.0 to 1.0).

```js
let stick = get_vr_thumbstick("left");
// stick.x = left/right, stick.y = up/down
```

### is_vr_button_pressed(hand, button)

Check if a specific face button is pressed. Button names:

| Name | Button |
|------|--------|
| `"a"` or `"x"` | Primary face button (A on right hand, X on left) |
| `"b"` or `"y"` | Secondary face button (B on right hand, Y on left) |
| `"trigger"` | Trigger (same as `is_vr_trigger_pressed`) |
| `"grip"` | Grip (same as `is_vr_grip_pressed`) |

```js
if is_vr_button_pressed("right", "a") {
    // A button pressed
}

if is_vr_button_pressed("left", "x") {
    // X button pressed
}
```

---

## Scripting API — Hand Tracking

### is_hand_tracked(hand)

Returns `true` if the specified hand is being tracked optically (no controller).

```js
if is_hand_tracked("right") {
    // Using bare hands
}
```

### get_hand_pinch_strength(hand)

Returns pinch gesture strength (0.0–1.0). Based on the distance between thumb tip and index tip.

```js
let pinch = get_hand_pinch_strength("right");

if pinch > 0.8 {
    // User is pinching — select something
}
```

### get_hand_grab_strength(hand)

Returns grab/fist gesture strength (0.0–1.0). Based on average fingertip-to-palm distances.

```js
let grab = get_hand_grab_strength("left");

if grab > 0.7 {
    // User is making a fist — grab something
}
```

---

## Scripting API — VR Commands

### vr_haptic_pulse(hand, intensity, duration)

Send a vibration pulse to a controller.

| Parameter | Type | Range | Description |
|-----------|------|-------|-------------|
| hand | String | "left" / "right" | Which controller |
| intensity | Float | 0.0–1.0 | Vibration amplitude |
| duration | Float | seconds | How long the pulse lasts |

```js
// Short, light tap
vr_haptic_pulse("right", 0.3, 0.05);

// Strong, sustained buzz
vr_haptic_pulse("left", 1.0, 0.5);
```

### vr_teleport_to(x, y, z)

Instantly move the player to a world position.

```js
vr_teleport_to(0.0, 0.0, -5.0);
```

### vr_recenter()

Reset the tracking origin so the player's current position becomes the center.

```js
vr_recenter();
```

### vr_set_passthrough(enabled)

Toggle passthrough mode from a script.

```js
// Enter mixed reality mode
vr_set_passthrough(true);

// Return to full VR
vr_set_passthrough(false);
```

---

## Scripting API — Component Properties

VR component properties can be read and written at runtime via the generic property system, just like any other component.

### VR Controller Properties

| Property | Type | Description |
|----------|------|-------------|
| `hand` | String | "left" or "right" |
| `show_laser` | Bool | Laser pointer visibility |
| `laser_length` | Float | Laser ray length (meters) |
| `show_model` | Bool | Controller model visibility |

### VR Grabbable Properties

| Property | Type | Description |
|----------|------|-------------|
| `grab_type` | String | "Snap", "Offset", or "Distance" |
| `throwable` | Bool | Can be thrown |
| `force_multiplier` | Float | Throw force scale |
| `max_grab_distance` | Float | Distance grab range |

### VR Teleport Area Properties

| Property | Type | Description |
|----------|------|-------------|
| `enabled` | Bool | Active state |
| `restrict_to_bounds` | Bool | Bound restriction |

### VR Hand Model Properties

| Property | Type | Description |
|----------|------|-------------|
| `hand` | String | "left" or "right" |
| `model_type` | String | "controller", "hand", or "custom" |
| `visible` | Bool | Visibility |

### VR Pointer Properties

| Property | Type | Description |
|----------|------|-------------|
| `hand` | String | "left" or "right" |
| `enabled` | Bool | Active state |
| `ray_length` | Float | Max ray distance (meters) |
| `show_cursor` | Bool | Hit point cursor visibility |

### VR Snap Zone Properties

| Property | Type | Description |
|----------|------|-------------|
| `snap_radius` | Float | Snap distance |
| `highlight_when_near` | Bool | Show highlight |
| `occupied` | Bool | (Read-only) Is an object snapped |

### VR Climbable Properties

| Property | Type | Description |
|----------|------|-------------|
| `enabled` | Bool | Active state |
| `grip_distance` | Float | Grab reach |

### Examples

```js
// Hide the controller model when hand tracking is active
if is_hand_tracked("right") {
    set_property(right_controller_id, "show_model", false);
} else {
    set_property(right_controller_id, "show_model", true);
}

// Disable a teleport area during combat
set_property(teleport_floor_id, "enabled", false);

// Check if a snap zone is occupied
let occupied = get_property(holster_id, "occupied");
if !occupied {
    // Highlight the empty holster
}
```

---

## VR Configuration Reference

All fields of the `VrConfig` resource, which backs the VR Settings panel:

| Field | Type | Default | Range | Description |
|-------|------|---------|-------|-------------|
| `render_scale` | f32 | 1.0 | 0.5–2.0 | Resolution multiplier per eye |
| `comfort_vignette` | f32 | 0.3 | 0.0–1.0 | Tunnel vision during locomotion |
| `snap_turn_angle` | f32 | 45.0 | 0.0–90.0 | Degrees per snap turn (0 = smooth) |
| `locomotion_mode` | enum | Teleport | Teleport/Smooth/Both | Movement method |
| `move_speed` | f32 | 2.0 | 0.5–10.0 | Smooth locomotion speed (m/s) |
| `hand_tracking_enabled` | bool | true | — | Enable optical hand tracking |
| `seated_mode` | bool | false | — | Use LOCAL reference space |
| `locomotion_hand` | enum | Left | Left/Right | Which hand moves |
| `thumbstick_deadzone` | f32 | 0.2 | 0.05–0.5 | Input dead zone |
| `snap_turn_cooldown` | f32 | 0.3 | — | Seconds between snap turns |
| `passthrough_enabled` | bool | false | — | Enable camera passthrough |
| `blend_mode` | enum | Opaque | Opaque/Additive/AlphaBlend | Environment blend mode |
| `foveated_rendering` | bool | true | — | Enable foveated rendering |

---

## Session Lifecycle

The VR session goes through a series of states managed by OpenXR:

```
Disconnected → Initializing → Ready → Focused ⇌ Visible → Stopping → Stopped
```

| State | Description | Rendering | Input |
|-------|-------------|-----------|-------|
| **Disconnected** | No headset / session not started | No | No |
| **Initializing** | OpenXR runtime starting up | No | No |
| **Ready** | Session created, waiting for headset to be worn | No | No |
| **Focused** | Headset on, app has input focus | Yes | Yes |
| **Visible** | Headset on, but system UI is overlaid (e.g., dash, guardian) | Yes | Limited |
| **Stopping** | Session shutting down | No | No |
| **Stopped** | Session ended | No | No |

The `VrSessionStatusChanged` event fires on every state transition. The VR Session panel shows a live timeline of recent transitions.

---

## Recipes

### Basic VR Scene Setup

1. Create a floor plane entity. Add **RigidBody** (Static) + **CollisionShape** (Box) + **VR Teleport Area**.
2. Create a cube entity. Add **RigidBody** (Dynamic) + **CollisionShape** (Box) + **VR Grabbable** (Offset).
3. Enter play mode with `--vr`. You can teleport to the floor and grab the cube.

### Gun with Haptic Feedback

```js
fn props() {
    #{
        fire_cooldown: #{ default: 0.0 }
    }
}

fn on_update() {
    fire_cooldown -= delta;

    if is_vr_trigger_pressed("right") && fire_cooldown <= 0.0 {
        let aim = get_vr_controller_position("right");
        spawn_projectile(aim.x, aim.y, aim.z);
        vr_haptic_pulse("right", 0.8, 0.05);
        fire_cooldown = 0.2;
    }
}
```

### Hand Tracking Pinch to Select

```js
fn on_update() {
    if is_hand_tracked("right") {
        let pinch = get_hand_pinch_strength("right");

        if pinch > 0.85 {
            // Perform selection at hand position
            let pos = get_vr_controller_position("right");
            select_nearest_entity(pos.x, pos.y, pos.z);
        }
    }
}
```

### Adaptive Controller/Hand Switching

```js
fn on_update() {
    let right_tracked = is_hand_tracked("right");
    let left_tracked = is_hand_tracked("left");

    // Switch hand model type based on tracking mode
    if right_tracked {
        set_property(right_hand_id, "model_type", "hand");
        set_property(right_hand_id, "show_model", true);
    } else {
        set_property(right_hand_id, "model_type", "controller");
    }
}
```

### Zone-Based Passthrough

```js
fn on_update() {
    let pos = get_vr_head_position();

    // Enable passthrough when player enters a specific area
    if pos.x > 5.0 && pos.x < 10.0 && pos.z > -5.0 && pos.z < 0.0 {
        vr_set_passthrough(true);
    } else {
        vr_set_passthrough(false);
    }
}
```

### Inventory with Snap Zones

1. Create empty entities at hip/back positions relative to the player. Add **VR Snap Zone** with `snap_radius: 0.15`.
2. Create weapon entities with **VR Grabbable** (Offset) + **RigidBody** (Dynamic).
3. When the player releases a weapon near the snap zone, it snaps into the holster. Grab it again to draw.

### Climbing Ladder

1. Create a ladder mesh entity.
2. Add **VR Climbable** with `grip_distance: 0.12`.
3. When the player grips near the ladder and moves their hand downward, they move upward. Release to stop climbing.

---

## Architecture Overview

### Crate Structure

```
crates/renzora_xr/
├── src/
│   ├── lib.rs           — XrPlugin, VrConfig, setup_xr_plugins(), reexports
│   ├── session.rs       — Session lifecycle, XrState → VrStatus mapping
│   ├── camera.rs        — VrCameraRig, VrHead, stereo camera setup
│   ├── input.rs         — sync_controller_state(), sync_hand_tracking()
│   ├── actions.rs       — OpenXR action sets, bindings, pose actions
│   ├── haptics.rs       — HapticPulseEvent, haptic output actions
│   ├── interaction.rs   — Teleport, smooth locomotion, snap turn, grab
│   ├── audio_bridge.rs  — Sync VR head position to audio listener
│   ├── passthrough.rs   — Passthrough toggle, blend mode
│   ├── reference_space.rs — Seated/standing reference space
│   ├── overlay.rs       — Overlay support
│   ├── extensions.rs    — VrCapabilities detection, extension management
│   ├── components.rs    — VR component data types (11 components)
│   └── resources.rs     — VrModeActive, VrSessionState, VrStatus
└── Cargo.toml
```

### System Execution Order

```
1. Startup
   └── setup_action_sets          (register OpenXR action sets and bindings)

2. XrSessionCreated (runs once when headset connects)
   ├── create_pose_actions        (manual OpenXR pose actions for grip/aim)
   ├── create_haptic_actions      (manual OpenXR haptic output actions)
   ├── configure_reference_space  (seated vs standing)
   └── detect_capabilities        (query supported extensions)

3. Update (every frame, only when VrModeActive)
   ├── update_session_state       (XrState → VrStatus, emit events)
   ├── spawn_vr_camera_rig        (create stereo cameras if needed)
   ├── sync_vr_head               (update head transform from HMD)
   ├── sync_controller_state      (read action states → VrControllerState)
   ├── sync_hand_tracking         (read bone entities → VrHandTrackingState)
   ├── process_haptic_events      (send vibrations to controllers)
   ├── teleport_system            (arc teleport)
   ├── smooth_locomotion_system   (thumbstick movement)
   ├── snap_turn_system           (discrete rotation)
   ├── grab_system                (pick up / release objects)
   ├── update_passthrough         (toggle passthrough on/off)
   ├── update_blend_mode          (set environment blend mode)
   └── sync_vr_head_to_audio_listener (spatial audio bridge)

4. PreScript (before Rhai execution)
   └── populate_vr_thread_locals  (push VR state into thread-local buffers)
```

### Key Resources

| Resource | Description |
|----------|-------------|
| `VrModeActive` | Marker resource. Present = VR is running. Used as run condition for all VR systems. |
| `VrConfig` | All user-facing VR settings. Persisted per project. |
| `VrControllerState` | Per-frame snapshot of both controllers (buttons, axes, poses) |
| `VrHandTrackingState` | Per-frame snapshot of both hands (26 joints each, pinch/grab strength) |
| `VrSessionState` | Session lifecycle (status, headset name, refresh rate, battery) |
| `VrCapabilities` | Which OpenXR extensions are available (hand tracking, passthrough, etc.) |
| `HapticPulseEvent` | Bevy Event — send to trigger controller vibration |
| `VrSessionStatusChanged` | Bevy Event — fires on session state transitions |

### Scripting Pipeline

```
VR ECS Resources
  → populate_vr_thread_locals()    (copies VR state to thread-local buffers)
  → Rhai scripts execute            (read from thread-local, push commands)
  → drain_vr_commands()             (collect commands from thread-local)
  → runtime.rs routes commands      (VrHapticPulse → HapticPulseEvent, etc.)
  → VR systems process events       (apply haptics, passthrough, teleport)
```

---

## Best Practices

### Performance

- **Target your headset's native refresh rate.** 90 Hz = 11.1 ms per frame, 120 Hz = 8.3 ms. Use the VR Performance panel to monitor.
- **Use Render Scale 1.0 as baseline.** Drop to 0.7–0.8 if frame budget is tight. Increase to 1.2–1.5 for visual quality when performance allows.
- **Enable foveated rendering** when available. It significantly reduces GPU load with minimal visual impact.
- **Watch for reprojection.** If the VR Performance panel shows frequent reprojection, reduce render scale, polygon count, or shader complexity.

### Comfort

- **Default to Teleport locomotion.** Smooth locomotion causes motion sickness for many users. Offer Both mode so users can choose.
- **Keep the comfort vignette at 0.2–0.4** for smooth locomotion. It significantly reduces nausea.
- **Use snap turning (30°–45°)** as the default. Smooth turning is an advanced option.
- **Never move the camera programmatically** without user input. Unexpected camera movement is the #1 cause of VR sickness.
- **Maintain stable frame rate.** Frame drops and reprojection artifacts cause discomfort.

### Interaction Design

- **Use Offset grab type as default.** It feels more natural than Snap for most objects.
- **Distance grab (force pull)** should be optional — it breaks immersion but is useful for accessibility.
- **Snap zones should have a visible indicator** so users know where to place objects.
- **Haptic feedback should be subtle.** Short pulses (0.02–0.1s) at moderate intensity (0.3–0.6). Reserve strong haptics for impactful moments.
- **Support both controllers and hand tracking** where possible. Use `is_hand_tracked()` to adapt.

### Scene Design

- **Ground plane at Y=0.** The VR tracking origin assumes the floor is at Y=0.
- **Scale matters.** 1 unit = 1 meter. A door should be ~2m tall, a table ~0.75m.
- **Avoid small text in world-space.** VR headsets have lower effective resolution than monitors. Use large, clear UI elements.
- **Test seated and standing.** Standing reference space (STAGE) places the origin on the floor. Seated (LOCAL) places it at head height.

---

## Troubleshooting

### "No headset detected" / Status stays Disconnected

- Is SteamVR running? VR requires an active OpenXR runtime.
- Is the headset connected (USB cable for Link, same network for Air Link)?
- Check SteamVR status — the headset should show as connected (green).
- Did you launch with `--vr`? Without it, VR systems are inactive.

### "Controllers not tracking" / All input is zero

- Check the VR Input Debug panel — are the tracked indicators green or red?
- Ensure controllers are powered on and connected to SteamVR.
- If using hand tracking, disable **Hand Tracking** in VR Settings to force controller mode.
- Check that you're in play mode (F5). Controller state is only synced during play mode.

### "Hand tracking not working"

- Is your headset supported? (Quest Pro/3 for optical hand tracking.)
- Is **Hand Tracking** enabled in VR Settings?
- Check the Capabilities section — does it show "Hand Tracking: Supported"?
- Ensure controllers are set down or turned off. Some headsets switch to hand tracking only when controllers are inactive.

### "Passthrough not showing"

- Check Capabilities — is "Passthrough: Supported"?
- Is the Passthrough checkbox enabled in VR Settings?
- Try switching Blend Mode to **Alpha Blend** — Opaque mode hides passthrough.
- Passthrough is a headset-specific feature. It won't work on headsets without cameras (e.g., original Vive, Rift CV1).

### "Teleport doesn't work"

- Does the floor entity have a **VR Teleport Area** component?
- Is the teleport area **Enabled**?
- Does the entity have a **CollisionShape**? The teleport system raycasts against colliders.
- Are you using the correct hand? Check which hand is set as the locomotion hand in VR Settings.

### "Grabbable object can't be picked up"

- Does the entity have both **VR Grabbable** and a **CollisionShape**?
- Is the entity within grab range? For Offset/Snap, the hand must touch the collider. For Distance, it must be within `max_grab_distance`.
- Check that the entity has a **RigidBody** (Dynamic). Static bodies can't be moved.

### "Performance is poor / constant reprojection"

- Open the VR Performance panel. Check the frame budget bar.
- Reduce **Render Scale** to 0.7–0.8.
- Enable **Foveated Rendering** if supported.
- Reduce scene complexity — fewer draw calls, lower-poly meshes, simpler shaders.
- Check CPU-side work — scripts running every frame can add up.

### "Build fails with cmake error"

- The `xr` feature requires cmake to build the native OpenXR SDK.
- Install cmake: `winget install Kitware.CMake` (Windows), `sudo apt install cmake` (Linux), `brew install cmake` (macOS).
- Ensure cmake is in your PATH. Restart your terminal after installation.

### "Build fails with openxr version conflict"

- The engine pins `openxr = "0.21"` to match `bevy_mod_openxr`. If you have a different version in your dependency tree, check for conflicting crates.
- Run `cargo tree -i openxr` to identify which crates depend on which version.
