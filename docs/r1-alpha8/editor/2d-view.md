# The 2D View

Pick **2D** in the viewport header to edit a 2D scene. Selecting any 2D node switches to it automatically, and it only switches back when you select something clearly 3D.

<!-- screenshot: viewport_2d.png - the 2D view with rulers, the amber camera boundary, and a sprite selected with its handles -->

## Getting around

Middle-mouse or right-mouse drag pans, and the scroll wheel zooms toward the cursor. `Shift` and scroll pans vertically, `Ctrl` and scroll pans horizontally. The header shows the zoom percentage.

**Rulers** along the top and left edges show world coordinates and track your cursor. They are on by default, and the cursor's coordinates also show in the status bar.

The **amber rectangle** is your game's camera boundary: exactly what a Camera 2D at the origin shows at runtime, taken from your project's viewport resolution. World (0, 0) is its top-left corner.

## The grid

Off by default. The **Grid** switch in the toolbar turns it on, and a number field beside it sets the cell size, defaulting to 16 world units to match the tilemap convention.

The grid draws behind your sprites so it never obscures the art, and it coarsens as you zoom out so it stays readable. Brighter section lines mark every eighth cell.

Its cell size is deliberately separate from the translate-snap step, so tuning snap never restyles the grid.

## Working with sprites

Drop an image from the **Assets** panel into the viewport to create a sprite at the cursor.

Selecting a sprite shows a frame with eight resize handles that follow the sprite's rotation, plus a rotate handle floating above the top edge. The cursor tells you what a drag will do.

- Drag the body to move, the handles to resize. `Shift` on a corner keeps the aspect ratio.
- Drag the rotate handle to spin. `Shift` snaps to 15 degree steps.
- `Shift+Click` adds to the selection, `Ctrl+Click` toggles, and dragging from empty space rubber-bands.

Dragging any sprite in a multi-selection moves the whole group rigidly, and arrow-key nudges move all of them.

Position and size are saved with the scene.

## Sprite components

**Flip X** and **Flip Y** on the Sprite Image component mirror the sprite. This is a render-side flip, so unlike a negative scale it leaves child entities, colliders and gizmos untouched.

**Sprite Sheet** crops the texture into a grid. **H Frames** and **V Frames** slice the image into columns and rows, and **Frame** picks which cell shows, row-major. The Frame field is animatable, so you can key it to play a flipbook. See [Sprite Animation](/docs/r1-alpha8/editor/sprite-animation).

## Y-sorting

For top-down scenes where a character should walk behind a tree when above it and in front when below it, switch on **Y Sort** on the Sprite Image component.

It derives draw order from world Y every frame: lower on screen means drawn in front.

| Field | What it does |
|---|---|
| Sort Offset | Moves the sort point away from the sprite's centre. A tall tree wants it at the trunk base, roughly minus half the sprite height. |
| Z Base | The layer the entity sorts within. Entities only sort against others with the same Z Base. |

Give your character the same offset treatment, sorting at the feet, and the crossover lands exactly where their footprints pass each other.

While a y-sorted entity is selected, a cyan line with a diamond marks its sort height. Two entities swap order exactly when their cyan lines cross, so you can tune Sort Offset against it live.

Y Sort owns the entity's Z from then on, recomputed every frame, so hand-set Z values will not stick.

Objects stamped from the [tilemap palette](/docs/r1-alpha8/editor/tilemap) come with Y Sort already on.

## Colliders

Select an entity with a **Collision Shape** and press the **Edit** toggle on its Inspector card. A green frame with eight handles appears over the collider, distinct from the orange sprite frame.

Drag a handle to resize it, or drag inside the shape to move its offset, which is how you trim a tree's collider down to its trunk. While the toggle is on, viewport clicks edit the collider instead of selecting sprites. Each drag is one undo step.

## 2D lights

2D lights always draw a small sun glyph in their own colour with a faint range ring, so an unselected light is findable without the Hierarchy. They respect the **Scene Icons** display toggle.

See [2D Lighting](/docs/r1-alpha8/rendering/2d-lighting).

## Playing a 2D scene

Press Play and the game renders through the 2D pipeline, framed to the game camera. What sits inside the camera boundary in the editor is exactly what shows on screen.

While the 2D view is active the editor parks the 3D pipeline, so 2D editing does not pay for bloom, anti-aliasing or global illumination. The reverse is also true.
