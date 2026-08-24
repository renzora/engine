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
| **Right-click + E / Q** | Fly up / down (slower the closer you are to the ground) |
| **Middle-click + drag** | Orbit around the focus point |
| **Shift + Right-click + drag** | Pan (slide the view sideways and up/down) |
| **Scroll wheel** | Zoom in / out |
| **Hold Ctrl while moving** | Move slowly, for fine adjustments |

The camera moves slowly when you're close to something and faster when you're far away, so navigating both tiny props and huge levels feels natural. `E` / `Q` have their own version of that: they ease off as you approach ground level, so you can settle onto the floor instead of punching through it, and open back up to full speed once you're clear of it.

> Tip: In **Edit mode** (mesh editing) the `E` and `Q` fly keys are used by the editing tools instead. WASD still flies — use scroll or Shift+Right-drag to move up and down.

### Handy camera shortcuts

| Key | What it does |
|-----|--------------|
| `F` | Focus on the selected object (centers the camera on it) |
| `A` | Frame All — fit the whole scene into view |
| `Home` | Reset the camera to its starting position |
| `End` | Move the focus point to wherever your cursor is pointing |
| `[` / `]` | Slow down / speed up the camera |

There's also a small button cluster on the right edge of each viewport for **Pan** and **Zoom** — press and drag them, dragging **up** on Zoom to move closer — and an **orientation gizmo** in the top-right corner that shows which way the camera is facing.

While you're dragging Zoom, a **height ruler** slides in on the right: a strip of ticks with a single white number on the centre line — your height, in **metres**. The scale picks itself from how high you are, so it reads the same whether you're a metre off the floor or a kilometre up, and it stops at **0 m**; nothing counts below the ground. The white bar down its right edge is the **zoom range**: the marker rides from the top (fully zoomed out) to the bottom (fully in) so you can see how much room is left before the drag stops moving, and it grows longer the higher you get. It fades out shortly after you let go. (The Grid and Scene Icons circles that used to sit under them are gone: both already have switches in the toolbar's Display and Gizmos dropdowns, and in *Settings → Viewport*.)

Along the **top edge of the viewport** runs its toolbar: the session actions — **Undo**, **Redo**, and **Save** — then the tool buttons (**Select / Move / Rotate / Scale**, the terrain modes **Sculpt / Paint Layers / Paint Foliage** — plus **Make Terrain** whenever the selection is a flat mesh, which [turns that plane into a terrain](terrain.md#making-a-terrain-out-of-a-plane) — mesh **Edit Mode** and **X Symmetry**, and any modes plugins add), the inline **snap steps** for move / rotate / scale (click the icon to toggle that snap, drag or type the number to set its step), the shape / display / gizmos / camera menus, **Play**, and this viewport's own view-angle and World/Local controls, left to right, wrapping onto another line when they need to. The **maximize** button is the exception: it floats hard against the **right edge** of the bar, whatever else is on it. It hides during play mode, all except Play, which becomes Stop and stays where it is.

The toolbar holds the buttons that say *what the viewport is set to do*. What each of those opens — the brushes, the select modes, the ops — is on the tool shelf down the left edge, described below. There's no Sculpt Mode button: the **Mode** dropdown beside the 3D/2D/UI selector already lists Scene / Edit / Sculpt, and one control for it is enough.

Below the toolbar, hard against the top of the scene, sits the **brush settings bar**: the active terrain brush's size, strength and falloff, its shape and falloff-curve toggles, and whatever that particular brush adds (Flatten's target height, Noise's octaves, Stamp's rotation). It appears only while a terrain brush is in hand, and it sits directly above the shelf's first button, so the brush you picked and the settings for it are next to each other.

## The tool shelf

Down the **left edge of the viewport** sits the **tool shelf**: a two-column palette of icon buttons, the shape image editors use for their brushes. It holds what the toolbar's modes *open*, stacked in groups separated by a rule, each group appearing only when it applies:

| Group | Shows when | Holds |
|---|---|---|
| Draw | Edit mode | Draw Box, Draw Polyline |
| Modeling — select | Edit mode | Vertex, Edge, Face, Loop Cut |
| Modeling — ops | Edit mode | Subdivide, Inset, Merge, Delete |
| Modeling — brushes | Sculpt mode | Draw, Smooth, Grab, Inflate, Flatten, Pinch |
| Terrain — whole | any terrain tool is in hand | Generate Terrain, Resize Terrain, Terrain Size & Resolution |
| Terrain — sculpt | Sculpt Terrain is the active tool | all 17 sculpt brushes |
| Terrain — paint | Paint Terrain Layers is active | Paint, Erase, Smooth, Fill |
| Foliage | Paint Foliage is active | Paint / Erase, then one button per foliage type |

Pick up the terrain sculpt tool in the toolbar and all 17 sculpt brushes are there at once; switch to terrain paint and it swaps to the paint brushes. Enter Edit mode and you get the two draw tools, the select modes, and the ops. The shelf collapses completely when nothing in it applies.

Every group is an even number of buttons, so none of them ends on a half-empty row — which is why Loop Cut sits with the select modes rather than with the ops (it's modal like they are: it arms and previews, where the four ops fire on click), and why **Generate Terrain** and **Resize Terrain** are on the shelf rather than in the toolbar's terrain row. Neither opens a palette of its own, so up there each was a mode button with nothing under it; here they sit together as the operations that act on the terrain *as a whole* — fill it with procedural mountains, drag its extent out, or type that extent in via **Terrain Size & Resolution**. Like everything else on the shelf, the group appears once a terrain tool is in hand: pick any terrain mode in the toolbar and the whole column comes up together, this group included.

The shelf exists because the top strip is the wrong shape for this. A handful of modes fits across it; seventeen brushes — or the ten buttons Edit mode wants — wrap it into a second row and push Play and the view controls down with them, and a row of identical squares is hard to hunt through. Down the left edge there is nothing competing for the space, and two columns keeps the palette a compact block instead of a ribbon running off the bottom of the view.

Clicking a brush never reaches the scene behind it — the shelf blocks the pointer, so a click on a tool can't also select an object or start a box-select.

Plugins add to it the same way they add to the top strip: register a `ToolEntry` in a `ToolSection::Shelf(group)` section. Groups stack in alphabetical order of that string. See *Extending → Plugins*.

## Different views of your scene

Want to line something up dead-on from the front or top? The numpad snaps the camera to straight-on views:

| Key | View |
|-----|------|
| `Numpad 1` | Front (add `Ctrl` for Back) |
| `Numpad 3` | Right (add `Ctrl` for Left) |
| `Numpad 7` | Top (add `Ctrl` for Bottom) |
| `Numpad 5` | Switch between perspective and flat (orthographic) |

The viewport header also has a **3D / 2D / UI** selector: **2D** switches the panel to the flat, orthographic 2D editor (see below), and **UI** opens the canvas where you build your game's interface with the [renzora_ember markup system](/docs/r1-alpha7/scripting/game-ui).

## The 2D view

Pick **2D** in the header selector (or select any 2D node — the viewport switches automatically) to edit a 2D scene. The auto-switch only leaves 2D when you select something clearly 3D (a mesh, 3D camera, or light) — selecting an ambiguous entity like a freshly dropped scene instance keeps the view where it is:

- **Rulers** along the top and left edges show world coordinates and track your cursor. Toggle them with the **Rulers** switch in the toolbar (on by default). The cursor's world coordinates show live in the **left side of the status bar** (next to "Ready") whenever the pointer is over the 2D view — with or without rulers. Turn the readout off under **Settings → Viewport → 2D Cursor Coordinates**.
- **Grid** — off by default; flip the **Grid** switch in the toolbar (it only appears in 2D view) to show it. The grid draws as faint lines *behind* your sprites, so it never obscures the art. Its **cell size** is the number input that appears next to the switch while the grid is on (default **16** world units, matching the tilemap tile convention) — it is its own setting, deliberately independent of the translate-snap step, so tuning snap never restyles the grid. The grid adapts to your zoom: it draws at the configured size when you're zoomed in and automatically coarsens (doubling the spacing as needed) as you zoom out, so it stays readable at any zoom level instead of vanishing — every drawn line sits on a multiple of the configured size. Slightly brighter *section* lines mark every 8th cell (toggle with **Subgrid**, in *Settings → Viewport*). The switch is independent of the 3D view's grid toggle.
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

### Dragging one in from the shape library

To place a shape somewhere other than the origin, **drag it out of the shape
library panel** and over the viewport. A solid ghost of the shape follows your
cursor, standing on whatever is under it — the ground plane, or the face of an
existing mesh, so you can stack a crate on a crate or stick one to a wall.
Release to drop it there; release outside the viewport to cancel. Clicking a
tile without dragging still adds it at the origin.

**The move snap applies while you drag.** With the toolbar's **move** snap
turned on, the ghost steps across the grid in whole snap increments rather than
sliding smoothly, so what you drop is already aligned — and it aligns the same
way the Move gizmo does, meaning a shape doesn't shift the first time you nudge
it afterwards. That includes edge snapping: it's the shape's bottom corner that
lands on the gridline, so a dropped cube fills a grid cell instead of straddling
the line through its middle. The shape lands exactly where the ghost was
standing.

## Dropping models in

Drag a `.glb`/`.gltf` from the asset browser over the viewport and the real
model — full materials, not a grey placeholder — appears under your cursor and
follows it until you let go. What you're placing is already the final entity;
releasing over the viewport commits it in place rather than despawning the
preview and spawning something new.

Models **stand on the ground**: the drop point is the ground plane under the
cursor, and the model is lifted so its lowest point rests there, not its origin.
That matters because a GLB's origin is wherever the exporter left it — often the
centre of the model, sometimes above it — so aligning on the origin buries or
floats half of what you import. The lift is measured from the model's *complete*
bounds, which means the editor waits for every mesh in the file to finish
loading before it settles; on a large model you may see it hold at the cursor for
a moment first. If some mesh can never report bounds, it gives up after about two
seconds and places the model from whatever it has, rather than leaving it
hanging.

## Display toggles

| Key | Toggle |
|-----|--------|
| `Alt + Z` | Wireframe mode |
| `Alt + Shift + Z` | Lighting on / off |
| `Ctrl + G` | Grid on / off |

> These use `Alt` so they don't clash with `Ctrl+Z` (undo). Note that `H` hides the selected object.

## The floor grid

The **Display** dropdown's **Grid** row has the on/off switch and a **−  /  +** pair beside it. Each press of `+` divides the grid into smaller squares, and `−` goes back the other way — powers of two, so the finer lines always fall on the coarser ones. The number between them is the divisor: `1` is the base grid, `4` draws sixteenth-squares.

It's a subdivision count rather than a cell size because this grid is infinite and unitless — there's nothing to measure a "16 units" against the way there is in the 2D view. (The old **Sub-grid** switch is no longer here: it only ever affected the *2D* editor's grid, and still lives in *Settings → Viewport*.)

## Choosing which gizmos are drawn

Next to the **Display** dropdown (the eye) in the viewport toolbar is the **Gizmos** dropdown (the bounding-box icon). Display controls what the *renderer* produces — visualization mode, mesh / textures / lighting / shadows, the grid. Gizmos controls what the *editor* draws on top of your scene:

| Group | Switch | What it hides |
|-------|--------|---------------|
| **Selection** | Bounding Box | The orange wireframe box around the selected object |
| **Scene** | Lights | Light falloff wireframes — point radius spheres, spot cones, the sun's direction arrow, area-light rectangles, probe boxes |
| **Scene** | Cameras | The selected camera's frustum wireframe and forward arrow |
| **Scene** | Scene Icons | The light-bulb / sun / camera glyphs (2D view; the 3D icon overlay is not drawn yet) |
| **Scene** | Labels | Entity name labels floating above each object (off by default) |
| **Rigging** | Skeleton | The octahedral bone meshes drawn over a selected rigged model |
| **Physics** | Colliders | Collision-shape wireframes, plus a **Selected Only** / **Always** choice below the switch |

Everything here is on by default except Labels, and each switch is saved with the project.

Two things worth knowing:

- **Colliders now have an off state.** The Selected Only / Always pair only ever decided *when* the wireframes appear. Turning the switch off hides them entirely; turning it back on returns to whichever of the two you were using. Picking either mode row also switches colliders back on.
- **Collider wireframes are cross-hatched.** Every hull draws diagonals as well as edges — an X across each face of a box or mesh AABB, an X across each of the four side panels of a capsule or cylinder, and two 45° great circles on a sphere. A bare edge wireframe sitting on top of the mesh it wraps reads as a jumble of unrelated lines, and a collider that matches a boxy mesh vanishes into that mesh's own silhouette; the diagonals give each face a visible surface so the collider reads as a solid volume. Colour still carries the body type — green static, orange dynamic, blue sensor.
- **Skeleton is the one to reach for on heavy rigs.** Bone gizmos are real meshes rebuilt every frame, so a densely-boned character costs more than the line-based gizmos. Turning it off while you work on something else is the cheapest win in the list.

The same switches live in **Settings → Viewport → Gizmos**, alongside the drag opacity and the all-viewports option. They're global, not per-viewport, even though the dropdown sits on the viewport's own tool strip.

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

Rotating and scaling pivot around the **base of the object's bounding box** — centred on X and Z, sitting on the bottom in Y — so an object turns and scales about the point where it meets the floor and stays standing instead of sinking through the surface. This holds even for imported models whose pivot was authored at the world origin. Prefer the middle? Turn off **Settings → Viewport → Gizmos → Gizmo at Object Base** and both the handles and the drag pivot move back to the bounding-box centre.

While you drag a rotate ring, the swept angle fills in as a pie sector with the **angle in degrees** printed beside it. With the toolbar's rotate snap on, that readout steps with the object rather than counting through every intermediate degree, so the number you see is always the rotation actually applied — which also means it stays at `0.0` until your drag reaches the first step. The same readout appears for the keyboard `R` rotate.

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

Selected objects get an orange wireframe **bounding box** so you always know what's picked. Working on something where the box is in the way? Turn it off under **Gizmos → Selection → Bounding Box** in the viewport toolbar. Whether the box draws through geometry or is depth-tested is a separate choice, under **Settings → Viewport → Gizmos → Boundary** (On Top / Depth Tested).

## The grid

The grid is the faint set of lines on the ground that helps you judge distance and keep things lined up. The center lines show the world axes (**X red, Y green, Z blue**), and the grid fades out in the distance — zoom out and more of it appears. Toggle it with `Ctrl+G`.

## Working with multiple viewports

You can open **up to four viewports at once** to set up a classic layout — perspective, front, top, and side all visible together. Each one looks at the same scene from its own angle.

The **active** viewport is whichever one your cursor is over, so camera controls and dragging always act on the view you're working in.

**Each viewport has its own toolbar** across its top edge with the controls that belong to that specific view:

- a **view-angle dropdown** — pick Perspective, Front, Back, Left, Right, Top, or Bottom for *that* viewport, so you can lay out the classic perspective / front / top / side quad and change any one without touching the others;
- a **World / Local** toggle that sets the transform gizmo's axes for that viewport independently;
- a **maximize** button that expands *that* viewport to fill the editor (click it again, or the maximize button on the now-full viewport, to restore your layout). Every viewport has one, floated to the right edge of its own toolbar — including the primary, whose maximize spent a while riding in the document-tab strip and moved back when that strip left the panel. It sits outside the toolbar's draggable groups, so rearranging those never moves it off the edge.

The viewport's **own tool strip**, flush along its top edge, now holds all of it: Select / Move / Rotate / Scale, undo / redo / save, the shape menu, the move / rotate / scale snap steps, the display / gizmos / snap / camera menus, **Play**, this viewport's view-angle and World/Local controls, and its maximize button out on the right edge. (There is no longer a shared *toolbar* strip under the top bar: every panel that had tools there — the code editor, the material graph, the blueprint graph — now carries them inside itself. The strip under the top bar is your open document tabs.)

It fills the bar from the left, and sits **above** the rendered scene rather than floating over it — so the scene starts below the bar and the axis gizmo, nav buttons and 2D rulers move down with it.

If the viewport is too narrow for everything, the toolbar **wraps onto a second line** (or a third) instead of hiding controls behind a menu. A group never splits across lines: one that doesn't fit moves down whole. Nothing is ever out of reach.

Each group has a small **grip** on its left. Hover it and the group lights up; drag it to move that group somewhere else on the bar — a blue marker shows where it will land, and the groups around it shift aside as you go. The controls themselves stay clickable throughout, so there's no mode to switch in and out of. **Your arrangement is saved with the project** and comes back the next time you open it.

The **Play** button and its target caret sit between the tools and the per-view controls — just left of the view-angle (Perspective) dropdown, centred with everything else. They stay put while the game runs: the tools and the per-view controls hide during play, but Stop is always exactly where Play was.

**The selection gizmo follows your cursor.** When you select something, the transform gizmo (and, in 2D, the selection outline and resize handles) shows in the viewport your cursor is in, sized to that view — so the other views stay clean. Prefer to see it everywhere at once? Turn on **Settings → Viewport → Gizmos in All Viewports**, and every viewport draws its own correctly-sized handle; dragging still happens in whichever viewport you're pointing at. The grid, and the orientation cube in each corner, always reflect each viewport's own camera.

This works in **2D** too: switch to the 2D view and every open viewport shows the 2D scene, each with its own independent **pan and zoom** — so you can keep one viewport framed on the whole level while another stays zoomed in on a character, each with its own grid. A newly opened 2D viewport starts on the same framing as the one you're working in, then pans and zooms independently from there. Interaction (select, paint, the tools) always follows the active viewport, exactly as in 3D.

## Previewing a camera shot

The **Camera Preview** panel shows the scene from one of your *game* cameras, so you can frame an in-game shot while you keep editing from a different angle. It previews, in order: a selected object that has a camera, your default camera, or the first camera it finds in the scene. The preview matches your scene's sky and lighting so it looks like the final result.

## Playing your game

Press **Play** to play-test your game without leaving the editor. Edit mode and play mode **share the viewport panel**: when you press Play, the viewport switches from your editor camera to the running game (seen through the active game camera), constrained to the panel — your hierarchy, inspector, console, and the rest of the editor all stay on screen. Press **Stop** (or `Esc`) and the viewport flips straight back to the editor camera, right where you left it.

- **Pressing Play brings the viewport tab to the front automatically**, so you see the game even if you were looking at another tab when you started.
- Entering play gives a clean game view: it **clears your selection and hides the editor toolbars, the axis gizmo, and the viewport buttons**; Stop brings them back. (The Play/Stop control itself lives in the top bar and stays put throughout.)
- **Maximize on Play** (Settings → Editor → Camera, **on by default**): pressing Play collapses the dock to just the viewport for a full-panel game view, and Stop restores your layout. Turn it off to keep the rest of your panels visible while playing.
- If no viewport panel is open at all, play falls back to taking over the whole window.
- The game's render resolution follows the active camera's resolution setting, just like the editor view.

> Input goes to the game globally while playing — keyboard and mouse reach your scripts even though the game is windowed. A script that grabs the cursor (e.g. an FPS look controller) grabs it for the whole editor window.

### Choosing where Play runs

The small **caret next to the Play button** opens the play-target menu:

- **Play in Viewport** (the default) — the in-editor experience described above: the game runs inside the viewport panel with the rest of the editor around it.
- **Play in Runtime Window** — Play launches the game as its **own process in its own OS window**, exactly like an exported build: the window uses your project's **Window settings** (Settings → Project → Window — title from the project name, resolution, windowed / fullscreen / borderless mode, resizable) and your window icon. The editor pauses behind a dark overlay while the game owns the screen, and wakes back up the moment you close the game window (or press **Stop**, which closes it for you).

**The button says where it will run.** Its label follows the selected target, so you can see the choice without opening the menu: **Play Viewport**, **Play VR**, **Simulate**, and plain **Play** for the runtime window (launching the game in its own window is what a play button ordinarily means). While the game is running it reads **Stop** as usual.

The choice is remembered across sessions (per-user, in `~/.renzora/editor.toml`) and every following Play uses it. The same switch also lives in **Settings → Scripting → External Window**.

A few things to know about the runtime window:

- Your scene is **saved to disk first** (same as regular Play), because the spawned runtime reads the project's files — it starts from the project's **main scene**, just like an exported game.
- The engine is **one binary**: the editor relaunches its own executable with `--no-editor --project <your project>`, which boots it straight into game mode. If a dedicated `renzora-runtime` binary is staged next to the editor (packaged `renzora build` output), that leaner binary is used instead — same result either way.
- Because it's a separate process, it's fully insulated from editor state: no editor cameras, gizmos, or overlays can leak in.
- **Its log appears in the editor's Console**, tagged `Runtime`, for as long as the game is running — everything it prints, including plugin-load failures and panics. The runtime is a windowed process with no terminal of its own, so without this its output goes nowhere and a game that misbehaves only outside the editor has nothing to show for it. The console also records how the run ended, and flags a non-zero exit code as an error.
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
