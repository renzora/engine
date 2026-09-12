# Viewport and Camera

The viewport is your live view of the scene. This page covers moving the camera, the tools along its edges, and what you can switch on and off.

![The Renzora 3D viewport showing a street scene with a scooter selected and the coloured Move gizmo attached to it.](/assets/previews/viewport.png)

## Moving the camera

| Input | What it does |
|---|---|
| Right-click and drag | Look around |
| Right-click and `W` `A` `S` `D` | Fly forward, back, left, right |
| Right-click and `E` / `Q` | Fly up and down |
| Middle-click and drag | Orbit the focus point |
| `Shift` and right-click drag | Pan |
| Scroll wheel | Zoom |
| Hold `Ctrl` while moving | Move slowly, for fine adjustments |

The camera moves slowly when you are close to something and faster when you are far away, so small props and large levels both feel natural. `E` and `Q` ease off near ground level so you can settle onto the floor rather than punch through it.

| Key | What it does |
|---|---|
| `F` | Frame the selection |
| `A` | Frame the whole scene |
| `Home` | Reset the camera |
| `End` | Move the focus point to wherever your cursor is pointing |
| `[` / `]` | Slow down, speed up |

An **orientation gizmo** in the top-right corner shows which way the camera is facing. Click one of its balls to snap to that axis, or drag it to orbit.

Below it sits a cluster of four camera controls: **Reset View**, **Pan**, **Zoom** and **Grid**. Press and drag Pan or Zoom. Grid lights up while the floor grid is on.

## Straight-on views

| Key | View |
|---|---|
| `Numpad 1` | Front, with `Ctrl` for Back |
| `Numpad 3` | Right, with `Ctrl` for Left |
| `Numpad 7` | Top, with `Ctrl` for Bottom |
| `Numpad 5` | Switch between perspective and orthographic |

The same views are in the camera menu on the toolbar.

<!-- screenshot: viewport_camera_menu.png - the camera dropdown showing perspective/orthographic and the six view angles -->

## The toolbar

Along the top edge of the viewport, left to right: **Undo**, **Redo** and **Save**, then the tool buttons, the snap steps, the shape, display, gizmo and camera menus, **Play**, and this viewport's own view-angle and World / Local controls. **Maximize** floats against the right edge.

<!-- screenshot: viewport_toolbar.png - the viewport toolbar with the Select, Move, Rotate and Scale tools and the snap controls -->

The tool buttons are **Select**, **Move**, **Rotate** and **Scale**, plus the terrain modes when a terrain is selected and anything plugins add.

The snap steps sit inline: click the icon to toggle that snap, then drag or type the number to set its step. Fractions are fine, so a quarter-unit grid is `0.25`. The magnet in the Snap menu lights up while any snap is on.

## The tool shelf

Down the left edge of the viewport is the tool shelf, a two-column palette of brushes and operations. It shows what the toolbar's modes open, and each group appears only when it applies.

| Group | Shows when | Holds |
|---|---|---|
| Terrain, whole | Any terrain tool is in hand | Generate Terrain, Resize Terrain, Terrain Size and Resolution |
| Terrain, sculpt | Sculpt Terrain is active | All 17 sculpt brushes |
| Terrain, paint | Paint Terrain Layers is active | Paint, Erase, Smooth, Fill |
| Foliage | Paint Foliage is active | Paint and Erase, then one button per foliage type |

Plugins can add their own groups. In the 2D view the shelf collapses entirely.

While a terrain brush is in hand, a **brush settings bar** appears under the toolbar with that brush's size, strength and falloff, plus whatever the brush adds of its own.

## The gizmo

Select an object and a set of coloured handles appears. Drag one to transform the object. The handles draw on top of the scene and stay a comfortable size however far away the camera is.

<!-- screenshot: gizmo_move.png - the move gizmo on a selected object -->
<!-- screenshot: gizmo_rotate.png - the rotate gizmo -->
<!-- screenshot: gizmo_scale.png - the scale gizmo -->

`Q` `W` `E` `R` switch between Select, Move, Rotate and Scale.

**World or Local** decides which axes the gizmo uses. World keeps the handles aligned to the scene. Local aligns them to the object's own rotation, which is what you want when nudging something along its own forward.

### From the keyboard

Press a key, move the mouse, click to confirm.

| Key | Action |
|---|---|
| `G` | Grab and move |
| `R` | Rotate |
| `S` | Scale |
| `X` / `Y` / `Z` | Lock to one axis, press again to clear |
| `Shift` and an axis key | Lock to the plane facing that axis |
| Numbers | Type an exact amount |
| `Enter` or left-click | Confirm |
| `Esc` or right-click | Cancel |

## The shading switch

Four buttons sit centred on the viewport's top edge. They are a ladder, each adding one thing to the one below.

<!-- screenshot: viewport_shading.png - the four shading buttons with Material active -->

| Mode | Adds | What you see |
|---|---|---|
| Wireframe | Topology | Edges only, on a flat dark background |
| Solid | Form | Neutral clay under matcap shading, no materials or scene lighting in the way |
| Material | Materials and lighting | Real materials and textures, lit, with shadows, against a flat background |
| Rendered | The world | Sky, atmosphere and clouds around them. The final look. |

These are presets over the same switches the Display menu holds, so change one by hand and no button is lit. Click one to get back.

**Only Rendered shows the world environment.** That is the point of having both it and Material: Material is where you judge materials and lighting without a sky colouring everything you are trying to read. Nothing in the scene is edited either way.

## Display

<!-- screenshot: viewport_display_menu.png - the Display dropdown showing Visualization, mesh, textures, lighting, shadows and overlays -->

The Display menu holds mesh, textures, lighting, shadows, the grid and the overlays.

Turning **Textures** off does not unlight the scene. Surfaces keep their real lighting and shadows and swap their maps for the blockout grid, so the scene reads as untextured geometry rather than geometry in the dark.

| Key | Toggle |
|---|---|
| `Alt+Z` | Wireframe |
| `Alt+Shift+Z` | Lighting |
| `Ctrl+G` | Grid |

### Visualization modes

The **Visualization** row replaces every material in the viewport with one that answers a single question: Normals, Roughness, Metallic, Depth, UV Checker or Matcap. **None** puts the real materials back.

These are a view, not an edit. Nothing is written to your materials and nothing ships.

**Matcap** is the one to reach for when you are judging a form. Its lights are fixed to the camera rather than the world, so orbiting tells you about the shape rather than about the lighting. It also shades creases by curvature rather than by direction, so fine detail that is invisible under lit shading shows up.

### The floor grid

The **Grid** row has an on/off switch and a **-** / **+** pair. Each press of **+** divides the grid into smaller squares, in powers of two, so finer lines always fall on the coarser ones. The number between them is the divisor. New projects start at `2`.

### Statistics

Switch on **Overlays > Statistics** for a small block of numbers in the bottom-left corner.

<!-- screenshot: viewport_stats.png - the statistics overlay in the corner of the viewport -->

| Row | Counts |
|---|---|
| Objects | Mesh instances the renderer is drawing. An imported model is usually several. |
| Verts | Total vertices |
| Tris | Total triangles. The number to watch when a scene feels heavy. |
| Height | Terrain only. The lowest and highest point of the actual ground, in metres. |
| Range | Terrain only. The envelope those heights are stored inside. |

The counts refresh four times a second. Clicks pass straight through to the scene behind.

## Gizmos

The **Gizmos** menu controls what the editor draws on top of your scene, as opposed to Display, which controls what the renderer produces.

| Group | Switch | Hides |
|---|---|---|
| Selection | Bounding Box | The orange wireframe box around the selection |
| Scene | Lights | Light falloff wireframes: point spheres, spot cones, the sun's arrow, probe boxes |
| Scene | Cameras | The selected camera's frustum and forward arrow |
| Scene | Scene Icons | Light bulb, sun and camera glyphs |
| Scene | Labels | Entity names floating above each object. Off by default. |
| Rigging | Skeleton | The bone meshes over a selected rigged model |
| Physics | Colliders | Collision wireframes, with a **Selected Only** or **Always** choice below |

Everything is on by default except Labels, and each switch is saved with the project.

Collider wireframes are cross-hatched so each face reads as a surface rather than a jumble of lines. Colour carries the body type: green static, orange dynamic, blue sensor.

**Skeleton** is the one to turn off on heavy rigs. Bone gizmos are real meshes rebuilt every frame, so a densely boned character costs more than the line-based gizmos.

The same switches are in **Settings > Viewport > Gizmos**.

## Selecting

Click an object to select it. `Ctrl+Click` adds or removes, `Shift+Click` extends a range, and dragging from empty space rubber-bands a selection. `Esc` clears it.

`H` hides the selection and `Shift+H` isolates it, hiding everything else.

## Adding shapes

The shapes dropdown at the left end of the toolbar lists every built-in primitive, grouped into Basic, Curved, Level and Advanced. Picking one drops it at the origin. The menu stays open so you can add several, and each add is one undo step.

It is the same list as the Shape Library panel and the Hierarchy's **Add Entity** menu.

New shapes arrive with a generated grid material on them, so you can see their form and scale before you have made any materials. Replace it by dropping a material on the object.

## Dropping models in

Drag a model from the **Assets** panel into the viewport and it spawns where you dropped it, sitting on whatever surface is under the cursor.

## Multiple viewports

You can open up to four viewports at once, each with its own camera and its own view angle. Add them from the **Add Panel** picker.

<!-- screenshot: viewport_multi.png - four viewports open at once showing different angles -->

Gizmo settings are global, so switching one switches all of them. Each viewport keeps its own camera, shading mode and view angle.

**Camera Preview** is a separate panel that shows what a selected camera sees, so you can frame a shot without flying there.

<!-- screenshot: camera_preview.png - the Camera Preview panel showing a selected camera's view -->

## If the viewport feels slow

Most of the cost of a frame is fullscreen image effects, and that cost grows with your display's resolution. On older laptops, integrated graphics or high-DPI screens the editor can feel sluggish even on an empty scene.

Open **Settings > Viewport > Performance > Graphics Quality** and drop it a notch.

| Tier | What it does |
|---|---|
| High | Everything on |
| Medium | Turns off screen-space global illumination, the single most expensive effect, keeping bloom, anti-aliasing and auto-exposure |
| Low | Turns those off too |

Medium is the default. The choice is saved per project.

## Related pages

- [Playing and Simulating](/docs/r1-alpha8/editor/play-mode)
- [The 2D View](/docs/r1-alpha8/editor/2d-view)
- [Terrain](/docs/r1-alpha8/editor/terrain)
