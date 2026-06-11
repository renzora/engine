# Viewport & Camera

The viewport is your live window into the 3D scene. You fly the camera around, click objects to select them, and drag colorful handles to move, rotate, and scale your world.

If you have ever used Blender, Unreal, or Unity, this will feel familiar. If you haven't — don't worry, you only need a few keys to get going.

![The Renzora 3D viewport showing a stylized Parisian street scene with a blue scooter selected and the colored Move gizmo arrows attached to it.](/assets/previews/viewport.png)

## Moving the camera

The camera orbits around a *focus point* and zooms in and out toward it. Start with these and you'll be comfortable in a minute:

| Input | What it does |
|-------|--------------|
| **Right-click + drag** | Look around |
| **Right-click + WASD** | Fly forward / back / left / right |
| **Right-click + E / Q** | Fly up / down |
| **Middle-click + drag** | Orbit around the focus point |
| **Shift + Right-click + drag** | Pan (slide the view sideways and up/down) |
| **Scroll wheel** | Zoom in / out |
| **Hold Ctrl while moving** | Move slowly, for fine adjustments |

The camera moves slowly when you're close to something and faster when you're far away, so navigating both tiny props and huge levels feels natural.

> Tip: In **Edit mode** (mesh editing) the `E` and `Q` fly keys are used by the editing tools instead. WASD still flies — use scroll or Shift+Right-drag to move up and down.

### Handy camera shortcuts

| Key | What it does |
|-----|--------------|
| `F` | Focus on the selected object (centers the camera on it) |
| `A` | Frame All — fit the whole scene into view |
| `Home` | Reset the camera to its starting position |
| `End` | Move the focus point to wherever your cursor is pointing |
| `[` / `]` | Slow down / speed up the camera |

There's also a small button cluster on the right edge of each viewport for **Pan** and **Zoom** (press and drag) plus **Grid** and scene-**Icons** toggles, and an **orientation gizmo** in the top-right corner that shows which way the camera is facing.

## Different views of your scene

Want to line something up dead-on from the front or top? The numpad snaps the camera to straight-on views:

| Key | View |
|-----|------|
| `Numpad 1` | Front (add `Ctrl` for Back) |
| `Numpad 3` | Right (add `Ctrl` for Left) |
| `Numpad 7` | Top (add `Ctrl` for Bottom) |
| `Numpad 5` | Switch between perspective and flat (orthographic) |

The viewport header also has a **3D / 2D / UI** selector: **2D** gives you a flat top-down camera for 2D games, and **UI** opens the canvas where you build your game's interface with the [renzora_ember markup system](/docs/r1-alpha5/scripting/game-ui).

## Display toggles

| Key | Toggle |
|-----|--------|
| `Alt + Z` | Wireframe mode |
| `Alt + Shift + Z` | Lighting on / off |
| `Ctrl + G` | Grid on / off |

> These use `Alt` so they don't clash with `Ctrl+Z` (undo). Note that `H` hides the selected object.

## Moving objects: the gizmo

When you select an object, a set of colored handles — the **gizmo** — appears on it. Drag a handle to transform the object. The handles always draw on top of your scene and stay a comfortable size no matter how far away the camera is.

Switch between gizmo tools with these keys:

| Key | Tool | Handles you'll see |
|-----|------|--------------------|
| `Q` | Select | None — just click to pick objects |
| `W` | Move | Colored arrows and plane squares |
| `E` | Rotate | Three colored circles |
| `R` | Scale | Colored lines with little cube caps |

The colors map to the 3D axes: **X is red, Y is green, Z is blue**. A handle turns **yellow** when you hover or drag it. (You can see the Move arrows on the selected scooter in the screenshot above.)

### Transform from the keyboard

If you'd rather not grab a handle, you can drive a transform straight from the keyboard with an object selected:

- Press `G` to **grab/move**, `R` to **rotate**, or `S` to **scale**.
- Press `X`, `Y`, or `Z` to lock to one axis.
- **Type a number** for an exact amount.
- Press **Enter** (or left-click) to confirm, **Escape** (or right-click) to cancel.

A small readout shows the current mode and any number you type.

## Selecting objects

| Input | What it does |
|-------|--------------|
| **Left-click** | Select the object under the cursor |
| **Shift + click** | Add an object to the selection |
| **Ctrl + click** | Toggle an object in or out of the selection |
| **Click + drag** | Box-select everything inside the box |
| **Click empty space** | Deselect everything |

Selected objects get a glowing outline and a bounding box so you always know what's picked.

## The grid

The grid is the faint set of lines on the ground that helps you judge distance and keep things lined up. The center lines show the world axes (**X red, Y green, Z blue**), and the grid fades out in the distance — zoom out and more of it appears. Toggle it with `Ctrl+G`.

## Working with multiple viewports

You can open **up to four viewports at once** to set up a classic layout — perspective, front, top, and side all visible together. Each one looks at the same scene from its own angle.

The **active** viewport is whichever one your cursor is over, so the gizmo and camera controls always act on the view you're working in. Switching to perspective or flat view applies to all of them at once; each viewport keeps its own angle.

## Previewing a camera shot

The **Camera Preview** panel shows the scene from one of your *game* cameras, so you can frame an in-game shot while you keep editing from a different angle. It previews, in order: a selected object that has a camera, your default camera, or the first camera it finds in the scene. The preview matches your scene's sky and lighting so it looks like the final result.
