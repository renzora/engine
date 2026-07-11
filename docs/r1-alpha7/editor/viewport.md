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

Along the **top edge of the viewport** runs its toolbar: the session actions — **Undo**, **Redo**, and **Save** — then the tool buttons (**Select / Move / Rotate / Scale**, the terrain tools, and any tools plugins add: draw box, draw polyline, tilemap paint, …), the inline **snap steps** for move / rotate / scale (click the icon to toggle that snap, drag or type the number to set its step), and a **maximize** toggle at the far right. It floats over the scene and hides during play mode.

## Different views of your scene

Want to line something up dead-on from the front or top? The numpad snaps the camera to straight-on views:

| Key | View |
|-----|------|
| `Numpad 1` | Front (add `Ctrl` for Back) |
| `Numpad 3` | Right (add `Ctrl` for Left) |
| `Numpad 7` | Top (add `Ctrl` for Bottom) |
| `Numpad 5` | Switch between perspective and flat (orthographic) |

The viewport header also has a **3D / 2D / UI** selector: **2D** switches the panel to the flat, orthographic 2D editor (see below), and **UI** opens the canvas where you build your game's interface with the [renzora_ember markup system](/docs/r1-alpha5/scripting/game-ui).

## The 2D view

Pick **2D** in the header selector (or select any 2D node — the viewport switches automatically) to edit a 2D scene:

- **Rulers** along the top and left edges show world coordinates and track your cursor. Toggle them with the **Rulers** switch in the toolbar (on by default). The cursor's world coordinates show live in the **left side of the status bar** (next to "Ready") whenever the pointer is over the 2D view — with or without rulers. Turn the readout off under **Settings → Viewport → 2D Cursor Coordinates**.
- **Grid** — off by default; flip the **Grid** switch in the toolbar (it only appears in 2D view) to show it. The grid draws as faint lines *behind* your sprites, so it never obscures the art. Its **cell size** is the number input that appears next to the switch while the grid is on (default **16** world units, matching the tilemap tile convention) — it is its own setting, deliberately independent of the translate-snap step, so tuning snap never restyles the grid. The grid adapts to your zoom: it draws at the configured size when you're zoomed in and automatically coarsens (doubling the spacing as needed) as you zoom out, so it stays readable at any zoom level instead of vanishing — every drawn line sits on a multiple of the configured size. Slightly brighter *section* lines mark every 8th cell (toggle with **Subgrid**). The switch is independent of the 3D view's grid toggle.
- The **amber rectangle** is your game's camera boundary — the exact area a Camera 2D at the origin shows at runtime, taken from the project's viewport resolution. World (0, 0) is its top-left corner, matching the runtime convention.
- **Middle-mouse or right-mouse drag** pans, the **scroll wheel** zooms toward the cursor, and the header shows the current zoom percentage. **Shift+scroll** pans vertically and **Ctrl+scroll** pans horizontally (a trackpad's sideways scroll always pans horizontally).
- **Selecting a sprite** shows a rotated-aware selection frame: the border and its eight resize handles follow the sprite's rotation. The cursor tells you what a drag will do — a **move** cursor over the sprite's body, **directional resize** cursors over the handles, and a **grab** cursor over the **rotate handle** (the circle floating above the top edge). Drag the rotate handle to spin the sprite; hold `Shift` to snap to 15° steps (the toolbar's rotate-snap step applies when its snap toggle is on).
- **Multi-select** — **Shift+click** adds a sprite to the selection, **Ctrl+click** toggles it in/out, and **dragging from empty space** sweeps a rubber-band box that selects everything it touches (hold `Shift` while banding to add to the current selection, `Ctrl` to toggle). Every selected sprite shows an outline; the primary keeps the resize/rotate handles. Dragging any sprite in a multi-selection **moves the whole group rigidly**, and arrow-key nudges move all of them.
- **2D lights** always draw a small sun glyph in their own colour (plus a faint range ring), so an unselected light is findable without the hierarchy. They respect the **Scene Icons** display toggle.
- **Drop an image** from the asset browser into the viewport to create a sprite at the cursor. Drag the selection's corner/edge handles to resize it (hold `Shift` on a corner to keep the aspect ratio). Sprite position **and size** are saved with the scene and restored on reload.
- **Flipping** — the **Sprite Image** component in the inspector has **Flip X** and **Flip Y** toggles that mirror the sprite horizontally or vertically. This is a pure render-side flip, so it mirrors only that sprite's art — unlike a negative Transform scale, it leaves child entities, colliders, and gizmos untouched. From a script, drive it with `set("Sprite.flip_x", true)` (e.g. face a character the way it's moving).
- **Sprite sheets** — to crop a sprite's texture into a grid of frames, add the **Sprite Sheet** component in the inspector. **H Frames** and **V Frames** slice the image into that many columns and rows, and **Frame** picks which cell shows (row-major, so frame = row × hframes + column; it wraps past the last cell). The grid is saved with the scene, and the `Frame` field is animatable from the [animation panel](/docs/r1-alpha7/editor/animation) — key it to play a flipbook.
- **Collider editing** — select an entity with a **Collision Shape** and press the **Edit** toggle on its inspector card: a green frame with eight handles appears over the collider (distinct from the orange sprite frame). Drag a handle to resize it, or drag inside the shape to move its offset — the way to trim a tree's collider down to its trunk. While the toggle is on, viewport clicks edit the collider instead of selecting sprites; each drag is one undo step.
- **Y-sorting** — for top-down scenes where a character should walk *behind* a tree when above it and *in front* when below it, flip the **Y Sort** toggle on the **Sprite Image** card in the inspector. It derives the entity's draw order from its world Y every frame: lower on screen = drawn in front. **Sort Offset** moves the sort point away from the sprite's centre — a tall tree wants it at the trunk base, so use roughly *minus half the sprite's height*; give your character the same treatment (sort at the feet) and the crossover point lands exactly where their footprints pass each other. **Z Base** is the layer the entity sorts within (default `1`, which draws above unsorted ground tiles at Z `0`); entities only y-sort against others with the same Z Base. While a y-sorted entity is selected, a **cyan line with a diamond** marks its sort height in the viewport — two entities swap draw order exactly when their cyan lines cross, so tune Sort Offset against it live. Objects stamped from the [tilemap palette](/docs/r1-alpha7/editor/tilemap) come with Y Sort already on, pivoting at their bottom edge. Y Sort owns the entity's Transform Z from then on — it's recomputed every frame, so hand-set Z values on y-sorted entities won't stick.

Pressing **Play** on a 2D scene renders the game through the 2D pipeline framed to the game camera's view, so what sits inside the camera boundary in the editor is exactly what shows on screen in play mode.

While the 2D view is active the editor parks the 3D render pipeline (its fullscreen passes rasterize into a token-sized buffer), so 2D editing doesn't pay for bloom, TAA, or global illumination — and vice versa: the 2D camera is off whenever you're in the 3D view.

## Adding shapes from the toolbar

The toolbar above the viewport carries a **shapes** dropdown (the multi-square icon, at its left end). Click it for a categorized list of every built-in primitive — **Basic** (cube, sphere, cylinder, plane, cone, capsule…), **Curved**, **Level** building blocks, and **Advanced**. Picking one drops it into your scene at the origin, ready to move with the gizmo. The menu stays open so you can add several in a row, and every add is a single undo step.

It's the same shape list as the shape-library panel and the hierarchy's **Add Entity** menu, so whatever you register shows up in all three.

## Display toggles

| Key | Toggle |
|-----|--------|
| `Alt + Z` | Wireframe mode |
| `Alt + Shift + Z` | Lighting on / off |
| `Ctrl + G` | Grid on / off |

> These use `Alt` so they don't clash with `Ctrl+Z` (undo). Note that `H` hides the selected object.

## If the viewport feels slow

Most of the cost of a frame is fullscreen image effects (global illumination,
auto-exposure, bloom, anti-aliasing), and that cost grows with your display's
resolution — so on older laptops, integrated GPUs, or high-DPI/Retina screens the
editor can feel sluggish even on an empty scene. Open **Settings → Viewport →
Performance → Graphics Quality** and drop it a notch:

- **High** — everything on (the full look).
- **Medium** *(default)* — turns off screen-space global illumination, the single
  most expensive effect, while keeping bloom, anti-aliasing, and auto-exposure.
- **Low** — turns those off too; the lightest, fastest mode for weak hardware.

The choice is saved per project. (For pinning down exactly *which* effect costs
you frames on a given machine, the **Render Toggles** debug panel — Add Panel →
Debug → Render Toggles — lets you flip each one live.)

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

Because the handles draw on top of everything, they'd normally hide the object as you drag it. To keep the object visible, the whole gizmo **fades to translucent while you're dragging a handle** and snaps back to fully opaque on release. How transparent it gets is up to you — set **Settings → Viewport → Gizmos → Drag Opacity** (`0` = invisible during the drag, `1` = no fade). The setting is saved per project.

Rotating and scaling pivot around the object's **bounding-box center**, so objects transform in place rather than drifting — this holds even for imported models whose pivot was authored at the world origin.

### World vs Local space

The **World / Local** icon button in the toolbar (next to the shapes dropdown — a **globe** in World space, a **cube** in Local; the tooltip names the active space) sets which axes the gizmo follows:

- **World** — handles align to the world axes (X/Y/Z), regardless of how the object is rotated.
- **Local** — handles align to the object's own orientation, so dragging moves it along *its* axes.

Either way the transform is applied correctly even when the object is nested under a rotated or scaled parent. Scale always acts along the object's own axes (the toggle only changes which way the scale handles point).

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

This works in **2D** too: switch to the 2D view and every open viewport shows the 2D scene, each with its own independent **pan and zoom** — so you can keep one viewport framed on the whole level while another stays zoomed in on a character. A newly opened 2D viewport starts on the same framing as the one you're working in, then pans and zooms independently from there. Interaction (select, paint, the tools) always follows the active viewport, exactly as in 3D.

## Previewing a camera shot

The **Camera Preview** panel shows the scene from one of your *game* cameras, so you can frame an in-game shot while you keep editing from a different angle. It previews, in order: a selected object that has a camera, your default camera, or the first camera it finds in the scene. The preview matches your scene's sky and lighting so it looks like the final result.

## Playing your game

Press **Play** to play-test your game without leaving the editor. Edit mode and play mode **share the viewport panel**: when you press Play, the viewport switches from your editor camera to the running game (seen through the active game camera), constrained to the panel — your hierarchy, inspector, console, and the rest of the editor all stay on screen. Press **Stop** (or `Esc`) and the viewport flips straight back to the editor camera, right where you left it.

- **Pressing Play brings the viewport tab to the front automatically**, so you see the game even if you were looking at another tab when you started.
- Entering play gives a clean game view: it **clears your selection and hides the editor toolbars, the axis gizmo, and the viewport buttons**; Stop brings them back.
- **Maximize on Play** (Settings → Viewport → Camera, **on by default**): pressing Play collapses the dock to just the viewport for a full-panel game view, and Stop restores your layout. Turn it off to keep the rest of your panels visible while playing.
- If no viewport panel is open at all, play falls back to taking over the whole window.
- The game's render resolution follows the active camera's resolution setting, just like the editor view.

> Input goes to the game globally while playing — keyboard and mouse reach your scripts even though the game is windowed. A script that grabs the cursor (e.g. an FPS look controller) grabs it for the whole editor window.

### Choosing where Play runs

The small **caret next to the Play button** opens the play-target menu:

- **Play in Viewport** (the default) — the in-editor experience described above: the game runs inside the viewport panel with the rest of the editor around it.
- **Play in Runtime Window** — Play launches the game as its **own process in its own OS window**, exactly like an exported build: the window uses your project's **Window settings** (Settings → Project → Window — title from the project name, resolution, windowed / fullscreen / borderless mode, resizable) and your window icon. The editor pauses behind a dark overlay while the game owns the screen, and wakes back up the moment you close the game window (or press **Stop**, which closes it for you).

The choice is remembered across sessions (per-user, in `~/.renzora/editor.toml`) and every following Play uses it. The same switch also lives in **Settings → Scripting → External Window**.

A few things to know about the runtime window:

- Your scene is **saved to disk first** (same as regular Play), because the spawned runtime reads the project's files — it starts from the project's **main scene**, just like an exported game.
- The engine is **one binary**: the editor relaunches its own executable with `--no-editor --project <your project>`, which boots it straight into game mode. If a dedicated `renzora-runtime` binary is staged next to the editor (packaged `renzora build` output), that leaner binary is used instead — same result either way.
- Because it's a separate process, it's fully insulated from editor state: no editor cameras, gizmos, or overlays can leak in.
- First launch can take a little while (the runtime loads the engine, plugins, and your project from cold); the editor shows its paused overlay until the game window appears.

## Simulate mode

The dropdown beside **Play** (the caret next to the Play button) picks what the Play button launches: **Viewport** (play in the editor), **Window** (play in a real runtime window), or **Simulate** (the blue flask) — pick it and the Play button turns into a blue **Simulate** button. Simulate runs the live simulation — scripts, physics, and animation all tick exactly as in Play — **but keeps the editor fully live**: your editor camera, gizmos, selection, and inspector stay active, and the camera does *not* switch to the game camera. It's the mode to reach for when you want to *watch and poke at* a running simulation rather than play it: triggering a ragdoll, watching physics settle, or testing a script's behaviour while still selecting and inspecting entities.

- **The viewport border turns green** while simulating, so it's always clear the scene is live and not just being edited.
- **Scripts take over the keyboard.** While simulating, editor keyboard shortcuts (and the editor-camera WASD) are suppressed so your scripts receive the keys — that's how a script's `is_key_pressed("KeyR")` sees input. You can still orbit the camera with the mouse to watch from any angle.
- **Stop restores the scene.** Simulate snapshots the scene on entry and reverts it on Stop (or `Esc`), so anything the simulation changed — moved bodies, a collapsed ragdoll, spawned or despawned entities — is undone and you're back exactly where you started. (Full **Play** does not restore; Simulate is the non-destructive option.)
- Like Play, Simulate needs a scene camera in the scene; the button is muted until one exists.
- While simulating, the button reads **Stop** (red) — click it (or press `Esc`) to end the simulation.
- The Simulate selection lasts for the editor session; the next launch starts back on Play (your Viewport-vs-Window choice is the part that's remembered).

> Because physics only runs while a simulation is live, features like the [ragdoll plugin](/docs/r1-alpha7/scripting/ragdoll) do nothing in plain edit mode — use **Simulate** (or **Play**) to see them move.
