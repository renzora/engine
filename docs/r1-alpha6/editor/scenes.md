# Scenes & Hierarchy

A **scene** is your game world: the characters, props, lights, and cameras that make up a level or a screen. You build it by arranging objects in the **Hierarchy** panel, then save it as a scene file you can reopen and ship.

This page walks you through that, no coding required.

## The Hierarchy panel

The **Hierarchy** is the list of everything in your current scene, shown as a tree. Anything you can see in the viewport has a row here.

![The Hierarchy panel showing a scene tree: an Add Entity button, a search box, and a filter funnel along the top, with a Terrain folder, World Environment, Camera, and an imported Bistro_Godot.glb model expanded into its mesh parts. Each row has an eye and a lock toggle on the right.](/assets/previews/hierarchy.png)

At the top you get three handy controls:

- **+ Add Entity** — drops a new object into the scene.
- **Search box** — type to filter the tree down to matching names.
- **Filter funnel** — narrow the list to one kind of object.

Below that is the tree itself. Click the little arrows (carets) on the left to expand or collapse a group, like the imported model in the screenshot above. On the right of each row are an **eye** (show/hide) and a **lock** toggle.

> Starting from scratch? When a scene is empty, the tree is replaced by a **starter picker** so you can begin from an Empty Scene, a 2D Scene, or other ready-made starters.

### Adding objects

Click **+ Add Entity** to open a searchable list, then pick what you want. The list gathers a few groups in one place:

- **Presets** — common scene objects: Empty Entity, Directional/Point/Spot/Ambient Light, Camera 3D, Camera 2D, Sprite, Node 2D.
- **Shapes** — ready-made meshes: Cube, Sphere, Cylinder, Plane, Cone, Torus, Capsule, and more.
- **Components** — add a single rendering, post-process, effect, or audio component as its own entity.

Installed plugins (physics, terrain, foliage, world environment) add their own entries too, so the list reflects whatever your project has loaded.

Just added the wrong thing? Press `Ctrl+Z` to undo it like any other action.

### Selecting objects

Click a row to select it. The viewport and the Inspector both follow your selection, so you immediately see and edit whatever you picked.

| What you do | What happens |
|---|---|
| Click | Select just this object |
| `Ctrl`+Click (or `Cmd`+Click) | Add or remove this object from the selection |
| `Shift`+Click | Select everything between the last pick and this one |
| Double-click | Rename it right in the tree |
| `Escape` | Deselect everything |

You can also **Select All**, **Hide Selected**, or **Isolate Selected** (hide everything except your selection) from the editor's actions.

### Parenting & reordering (drag and drop)

Drag a row to move it. Where you drop it decides what happens, based on the part of the target row you hover over:

- **Top of a row → Before** — move it just above that row.
- **Bottom of a row → After** — move it just below that row.
- **Middle of a row → As Child** — tuck it *inside* that row as a child.

Children move with their parent: rotate or move a parent and its children follow. The editor won't let you drop an object into one of its own children, and the whole move is one undo step.

Right-click a row for more options: **Add Child Entity**, **Rename**, **Duplicate**, **Unparent**, **Group as Children**, **label color** swatches, **Delete**, and (for cameras and scene instances) a few extra commands covered below.

### Showing, hiding & color-coding

Each row's **eye** toggles whether the object is visible, and the **lock** prevents accidental edits. Both are undoable. To keep a busy scene organized, give related objects a **label color** from the right-click menu.

## Moving, rotating & scaling

Select an object and a gizmo appears so you can move it around. Switch tools with these keys:

| Key | Tool |
|---|---|
| `Q` | Select (no gizmo) |
| `W` | Move |
| `E` | Rotate |
| `R` | Scale |

### Blender-style quick transforms

Prefer to work with the keyboard? Press a key, move the mouse, then click to confirm:

| Key | Action |
|---|---|
| `G` | Grab / move |
| `R` | Rotate |
| `S` | Scale |
| `X` / `Y` / `Z` | Lock to one axis (press again to clear) |
| `Shift`+`X`/`Y`/`Z` | Lock to the flat plane facing that axis |
| Type numbers | Enter an exact amount |
| `Enter` or left-click | Confirm |
| `Escape` or right-click | Cancel |

## Saving your scene

Press `Ctrl+S` to save. Scenes are stored as `.ron` files, kept by convention in your project's `scenes/` folder. You almost never edit these by hand — the editor writes them for you — but they are plain text if you ever want to peek.

Your project picks which scene loads first when the game runs. That's set in `project.toml` with a single `main_scene` line:

```toml
name = "My Game"
version = "0.1.0"
main_scene = "scenes/main.ron"
```

You can also list **autoload** scenes that load *before* the main scene and stay alive the whole time — handy for a loading screen, background music, or saved game state that should never be cleared:

```toml
main_scene = "scenes/main.ron"
autoload = ["scenes/loader.ron"]
```

> Good to know: only objects with a name are saved, and the editor leaves out runtime-only data (like rebuilt physics colliders and render handles) because the engine recreates it automatically when the scene loads. For the full technical breakdown, see [Project Structure](/docs/r1-alpha5/setup/project-structure) and [Components](/docs/r1-alpha5/engine-core/components).

## Reusing scenes inside scenes

You can drop one scene inside another as a **scene instance** — great for a prop, an enemy, or a room you want to reuse in many places. Edit the original once and every copy updates.

To add one, right-click in the Hierarchy and choose **Instance Scene…**, then pick a `.ron` file. Its contents appear nested under a new instance row. Choose **Unpack Scene Instance** if you'd rather break it apart into normal objects.

When you save, only the instance's own position and overrides are stored in the host scene — its insides still live in the original file, and any edits you make to them are saved back there. The editor also blocks a scene from referencing itself, so you can never create an endless loop.

## Handy scene shortcuts

These are the everyday shortcuts (all rebindable in **Settings → Keybindings**):

| Shortcut | Action |
|---|---|
| `Ctrl+N` | New scene |
| `Ctrl+O` | Open scene |
| `Ctrl+S` | Save scene |
| `Ctrl+Shift+S` | Save scene as… |
| `Ctrl+D` | Duplicate selected |
| `Alt+D` | Duplicate & move |
| `Delete` | Delete selected |
| `Ctrl+Z` | Undo |
| `Ctrl+Y` | Redo |
| `F` | Focus selected in viewport |

See [Keyboard Shortcuts](shortcuts.md) for the full, categorized list.
