# Scenes and Hierarchy

A scene is your game world: the characters, props, lights and cameras that make up a level or a screen. You build one by arranging objects in the **Hierarchy**, then save it as a file you can reopen and ship.

## The Hierarchy panel

Everything in the current scene, as a tree.

![The Hierarchy panel showing a scene tree: an Add Entity button, a search box and a filter funnel along the top, with Terrain, World Environment, Camera and an imported model expanded into its parts. Each row has an eye and a lock toggle.](/assets/previews/hierarchy.png)

Along the top: **+ Add Entity**, a search box that filters by name, and a filter funnel that narrows the tree to one kind of object.

<!-- screenshot: hierarchy_filter.png - the Hierarchy filter funnel menu open -->

On the right of each row are an **eye** to hide the object and a **lock** to stop accidental edits. Both are undoable.

## Adding objects

Click **+ Add Entity** for a searchable list.

![The Add Entity overlay: search or browse categories such as Lighting and Camera.](/assets/previews/add_entity.png)

| Group | Holds |
|---|---|
| Presets | Empty Entity, Directional, Point, Spot and Ambient Light, Camera 3D, Camera 2D, Sprite, Node 2D |
| Shapes | Cube, Sphere, Cylinder, Plane, Cone, Torus, Capsule and more |
| Components | A single rendering, post-process, effect or audio component as its own entity |

Installed plugins add their own entries, so the list reflects what your project has loaded.

To skip the search, right-click the empty space below the tree. You get the same list as a menu of categories. Hover one and its entities appear beside it, so a Point Light is a hover and a click away.

Added the wrong thing? `Ctrl+Z`.

## Selecting

| What you do | What happens |
|---|---|
| Click | Select just this object |
| `Ctrl+Click` | Add or remove this object from the selection |
| `Shift+Click` | Select everything between the last pick and this one |
| Double-click | Rename it in the tree |
| `↑` / `↓` | Move to the previous or next visible row |
| `→` / `←` | Open or close the selected branch, then step into or out of it |
| Drag from empty space | Rubber-band select |
| `Esc` | Deselect everything |

The arrow keys work once you have clicked into the Hierarchy, and they walk what you can actually see. A collapsed group is stepped past, not through.

Selecting a row also expands it, and clicking it again to deselect folds it back. Turn that off under **Settings > Interface > Hierarchy > Toggle on Click** if you are clicking through a deep model and do not want every row unfolding.

## Parenting and reordering

Drag a row to move it. Where you drop decides what happens:

| Drop on | Result |
|---|---|
| The top of a row | Move it just above that row |
| The bottom of a row | Move it just below |
| The middle of a row | Tuck it inside as a child |

Children move with their parent. You cannot drop an object into one of its own children, and the whole move is one undo step.

Right-click a row for **Add Child Entity**, **Rename**, **Duplicate**, **Unparent**, **Group as Children**, **Attach**, label colours and **Delete**.

## Attaching assets

### Creating one on the spot

Hover **Attach** in a row's right-click menu and you get the same new-file list the Assets panel offers: Rust Script, Material, Particle, Template, Scene.

An overlay asks for a **name**, pre-filled with a sensible default, and a **destination folder**, starting on the conventional folder for that type and creating it if you do not have one. An existing file is never overwritten.

For scripts there is also an **Attach to this object** tick, on by default, so a new script is wired up in one step.

### Attaching one you already have

Drag a file from the **Assets** panel onto the object's row. The row lights up as you hover it.

- A **script** is added to the object's Scripts component.
- A **material** is assigned to the object.

The object is selected after the drop, so the Inspector is already showing what you attached. Dropping the same script twice does nothing the second time.

If the row is an imported model's root, a material is applied to every mesh inside it, which is what people usually mean. Drop on a specific child row to change just that mesh.

Dropping a **scene** file instances it instead. See below.

### Asset badges

Objects carrying authored assets show a small badge left of the eye and lock, so you can see what is attached without opening the Inspector. Click a badge to open that asset in its editor: a code icon opens the script, a palette icon opens the material graph.

## Moving things around

Select an object and a gizmo appears. `Q` `W` `E` `R` switch between Select, Move, Rotate and Scale.

There are also Blender-style modal transforms: press `G`, `R` or `S`, move the mouse, click to confirm. `X` `Y` `Z` lock to an axis, `Shift` plus an axis key locks to the plane facing it, and you can type an exact amount. `Esc` cancels.

Full detail is in [Viewport and Camera](/docs/r1-alpha8/editor/viewport).

## Saving

`Ctrl+S` saves the scene. Scenes are `.bsn` files, kept by convention in your project's `scenes/` folder. They are plain text, but the editor writes them for you.

Only named objects are saved. Everything you add through **+ Add Entity** is named for you, so this rarely comes up.

### Scene thumbnails

Every save also snapshots the viewport and keeps it as the scene's thumbnail, so the Assets panel shows you the level rather than a generic icon. Whatever the focused viewport was showing when you saved is the picture you get. Frame the shot, then save.

A scene that has never been saved from the editor keeps its plain icon. This is the only moment the picture is free to take, because reproducing it later would mean loading the whole scene.

### Auto-save

On by default, every 5 minutes. Change it under **Settings > Auto-Save**.

A countdown appears in the status bar in the last few seconds. When it fires it saves the same way `Ctrl+S` does, so a focused material or script tab is never overwritten by it. Auto-save pauses while you are in Play mode.

## Which scene loads first

**Settings > Project > Boot Scene** picks the scene a *game* starts on. It is independent of whatever you are editing. Every save targets the scene tab you have focused, and nothing writes to the boot scene unless that is the tab you are on.

### Global scenes

A global scene loads before the boot scene and stays alive through every later scene change. Use one per concern: a HUD scene, a music scene, a networking scene.

Set them under **Settings > Project > Global Scenes**.

Everything a global scene spawns is marked persistent, so scene loads skip it and scene saves never bake it into the level you have open. Play and Simulate load them, and Stop unloads them, so you can test without exporting.

This is also the only place a loading screen can work, because everything in the outgoing scene is despawned partway through a scene change. Only a global scene's script is still running to show progress.

## Reusing scenes inside scenes

Drop one scene inside another as a **scene instance**. Good for a prop, an enemy or a room you want in many places. Edit the original once and every copy follows.

Three ways to add one:

- Drag a `.bsn` file from the Assets panel onto the **viewport**. It lands under your cursor.
- Drag it onto the **Hierarchy**. It is added at the scene root.
- Right-click in the Hierarchy and choose **Instance Scene**.

Its contents appear nested under an instance row. **Unpack Scene Instance** breaks it apart into ordinary objects.

When you save, only the instance's own position and overrides are stored in the host scene. Its insides still live in the original file, and edits to them are saved back there. A scene cannot reference itself.

## Shortcuts

`Ctrl+N` new, `Ctrl+O` open, `Ctrl+S` save, `Ctrl+Shift+S` save as, `Ctrl+D` duplicate, `Delete` delete, `F` focus the selection.

The full list is in [Keyboard Shortcuts](/docs/r1-alpha8/editor/shortcuts).
