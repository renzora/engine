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

### Expanding & collapsing

There are three ways to fold a branch open and shut:

- **The caret** on the left of any row with children.
- **The `←` / `→` arrow keys**, once you've clicked into the Hierarchy. `→` opens the selected branch, and — when it's already open — steps into its first child. `←` shuts an open branch, and steps out to the parent when there's nothing left to shut. With `↑`/`↓` for moving between rows, that's a whole imported model walked without touching the mouse. All four only apply while the Hierarchy is the panel you last clicked in, so they stay free for nudging a sprite in the viewport or stepping frames in the Timeline.
- **Clicking the row itself** — selecting an object also opens it, and clicking it again to deselect folds it back up.

That last one is a matter of taste, so it's a setting: **Settings → Interface → Hierarchy → Toggle on Click**, on by default. Turn it off when you're clicking through a deep model and don't want every row you touch unfolding under you — the caret and the arrow keys stay, and clicking becomes purely about selection.

> Starting from scratch? When a scene is empty, the tree is replaced by a **starter picker** so you can begin from an Empty Scene, a 2D Scene, or other ready-made starters.

### Adding objects

Click **+ Add Entity** to open a searchable list, then pick what you want. The list gathers a few groups in one place:

- **Presets** — common scene objects: Empty Entity, Directional/Point/Spot/Ambient Light, Camera 3D, Camera 2D, Sprite, Node 2D.
- **Shapes** — ready-made meshes: Cube, Sphere, Cylinder, Plane, Cone, Torus, Capsule, and more.
- **Components** — add a single rendering, post-process, effect, or audio component as its own entity.

Installed plugins (physics, terrain, foliage, world environment) add their own entries too, so the list reflects whatever your project has loaded.

**Or skip the search entirely**: right-click the **empty space below the tree** and you get the same list as a menu of categories — Lighting, Camera, Basic, Physics, Effects and the rest. Hover one and its entities appear beside it, so a Point Light is a hover and a click away with nothing typed. Each category carries the same accent color it has in the search overlay — amber for Lighting, violet for Camera, and so on — and its entities inherit it, so you can find a category by color once you know it. (Right-clicking a *row* keeps its own menu of actions for that object — see below.)

Just added the wrong thing? Press `Ctrl+Z` to undo it like any other action.

### Selecting objects

Click a row to select it. The viewport and the Inspector both follow your selection, so you immediately see and edit whatever you picked.

| What you do | What happens |
|---|---|
| Click | Select just this object |
| `Ctrl`+Click (or `Cmd`+Click) | Add or remove this object from the selection |
| `Shift`+Click | Select everything between the last pick and this one |
| Double-click | Rename it right in the tree |
| `↑` / `↓` | Move the selection to the previous / next visible row |
| Drag from empty space | Rubber-band select every row the box sweeps over |
| `Escape` | Deselect everything |

The `↑`/`↓` keys work once you've clicked into the Hierarchy, and they walk what you can actually see — a collapsed group is stepped past rather than through, and the list scrolls to follow the selection when it reaches an edge. Because the tree has focus it takes the keys back from arrow-key panel scrolling; click somewhere else and they go straight back to it. (`Shift`+`↑`/`↓` doesn't extend the selection — use `Shift`+click for a range.)

To **drag-select**, press in the empty area of the tree (below the last row) and drag a box over the rows you want — everything it touches is selected, and the list scrolls automatically when you reach its top or bottom edge. Hold `Ctrl` or `Shift` while you start the box to add to the current selection instead of replacing it. A plain click in that empty space clears the selection.

You can also **Select All**, **Hide Selected**, or **Isolate Selected** (hide everything except your selection) from the editor's actions.

### Parenting & reordering (drag and drop)

Drag a row to move it. Where you drop it decides what happens, based on the part of the target row you hover over:

- **Top of a row → Before** — move it just above that row.
- **Bottom of a row → After** — move it just below that row.
- **Middle of a row → As Child** — tuck it *inside* that row as a child.

Children move with their parent: rotate or move a parent and its children follow. The editor won't let you drop an object into one of its own children, and the whole move is one undo step.

Until you reorder them by hand, top-level objects sit in the order you added them — so anything you create lands at the bottom of the tree, which is where to look for it. Dragging a top-level row rearranges them for the rest of the session, and new objects still join at the end. That manual arrangement is editor-session state, though: it isn't written into the scene file, so reopening the scene puts the top level back in the order the file lists.

Right-click a row for more options: **Add Child Entity**, **Rename**, **Duplicate**, **Unparent**, **Group as Children**, **Attach ▸**, **label color** swatches, **Delete**, and (for cameras and scene instances) a few extra commands covered below.

### Attaching a new asset from the Hierarchy

Hover **Attach** in a row's right-click menu and you get the same list of new files the Assets panel's **Add** button offers — **Lua Script**, **Blueprint**, **Material**, **Particle**, **Template**, **Scene (BSN)** — each in its own type color. Files start with sensible contents rather than blank: a new **Blueprint**, for instance, already has its **On Ready** and **On Update** event nodes placed.

Pick one and an overlay asks the two questions that matter:

- **Name** — pre-filled with a sensible default (`new_script`, `NewMaterial`, …). Type your own; the extension is added for you, and an existing file is never overwritten (you'd get `player 2.lua`).
- **Destination folder** — your project's own folder tree, the same picker the marketplace installs assets into. It starts on the conventional folder for that type (`scripts/`, `materials/`, `particles/`, `blueprints/`, `ui/`, `scenes/`), creating it if your project doesn't have one yet, but you can drop the file anywhere — including the project root.

For **scripts and blueprints** there's also an **Attach to &lt;object&gt;** tick, on by default: the new file is added to the object's Script component as it's created, so a fresh script is wired up in one step instead of a round-trip through the Assets panel and a drag onto the Inspector. Untick it to just create the file.

`Escape`, the backdrop, or **Cancel** closes the overlay without creating anything.

### Attaching an existing asset by dropping it on a row

To attach a file you already have, drag it out of the **Assets** panel and drop it straight onto the object's row in the Hierarchy. The row lights up as you hover it, so you can see exactly which object you're about to hit before you let go:

- a **Lua script** (`.lua`) — added to the object's Script component;
- a **Blueprint** (`.blueprint`, `.bp`) — added the same way (blueprints run through the Script component too);
- a **Material** (`.material`) — assigned to the object.

The object is selected after the drop, so the Inspector is already showing what you just attached, and its new **asset badge** appears on the row. Each drop is a single undo step.

Dropping the same script on an object twice does nothing the second time — you don't end up running it twice by accident. A **multi-select drag** of several scripts attaches all of them in one go; a mixed drag only attaches the files matching the one you actually grabbed.

**Materials and meshes.** If the row you drop on is a **model root** — an imported model that carries the name but no mesh of its own — the material is applied to **every mesh inside it**, which is what "put this material on that model" nearly always means. Drop it on a specific child row instead to change just that mesh. If the object has no mesh anywhere beneath it, nothing is applied and the editor tells you so.

Scene files (`.bsn`, `.ron`) behave differently: dropping one anywhere in the panel [instances it into the scene](#reusing-scenes-inside-scenes) rather than attaching it to a row.

### Showing, hiding & color-coding

Each row's **eye** toggles whether the object is visible, and the **lock** prevents accidental edits. Both are undoable. To keep a busy scene organized, give related objects a **label color** from the right-click menu.

Just left of the eye and lock you'll also see small **asset badges** when an object carries authored assets, so you can tell what's attached without opening the Inspector — and **click a badge to jump straight to that asset's editor**:

- a **code** icon — the object has a script (a `.lua`/`.rs` file or a registered script); clicking opens it in the **code editor**;
- a **blueprint** icon — it has a visual blueprint (a `.blueprint` attached to its Script component); clicking opens the **blueprint graph**;
- a **palette** icon — it has a material assigned (a `MaterialRef`); clicking opens the **material graph**.

An object can show several badges at once (e.g. both a script and a blueprint).

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

## Working with document tabs

The strip of tabs **directly under the top bar**, running the full width of the window, is your open documents — one per scene, plus any materials, scripts, shaders, particles, or blueprints you've opened. Each is a square chip with an icon for what it is (a film slate for a scene, a palette for a material, a code glyph for a script, and so on) and the document's name. The icon carries its **type's colour** — the same blue, green, orange or purple the Assets browser gives that kind of file on its tiles — and it keeps that colour whether the tab is active or not, so the strip tells you what each open document *is* at a glance. Which tab is current is said by the accent rule under it and its brighter name, not by the icon. Tabs sit flush against each other, with no gap and no rounding. Both states are soft vertical gradients that land on the colour of whatever is beneath them: the active tab carries a breath of the accent under its top rule and fades into the panel colour it shares with the toolbar below, so it reads as *cut out* of the strip rather than highlighted on top of it; the inactive ones settle into a recessed shade, separated by a short hairline centred on each boundary. Inactive tabs are the plain ones — icon and name, no close button. A name too long for a tab is cut short with an ellipsis (`Big Level Sce…`) — hover it for the full name. You can keep several scenes open at once and click between them; each remembers its own contents and camera.

> **The strip is part of the window's chrome**, so it's on screen in every workspace. It spent a while inside the viewport panel, which meant it was missing from the five built-in workspaces that have no viewport (Blueprints, Materials, Particles, Animation, Hub) — and opening a material sends the editor to one of those, so the bar holding that material's tab disappeared the moment you clicked it.

**Don't want to spend a row on it?** *Settings → Appearance → Interface → UI Workspace → Document Tabs* switches the strip for a **Top Bar Dropdown**: a single button beside **Play** showing the document you're in — its icon, its name, and the `*` when it has unsaved edits — that opens onto all of them — the same list the strip shows, one per row — with the same **`+`** beside it for a new scene. The row goes back to the dock.

Every row in that menu carries its own **×**, unlike the strip where only the active tab does: the menu is the only way at a document you aren't currently in, so needing to switch to one just to close it would be backwards. Closing from a row leaves the menu open, so you can clear several in a go, and a document with unsaved changes still brings up the save prompt first. The last scene keeps no ×, exactly as in the strip. The choice is per-user (`~/.renzora/editor.toml`), so it survives restarts and doesn't travel with the project.

The bar spans the window, so nothing folds away until the tabs genuinely fill it. The `+` button sits directly after the last tab and moves right as you open more; once they fill the bar the extras fold into a **caret button** (`⌄`) at the end: click it for a menu of the tabs currently hidden, and picking one activates it as if you'd clicked the tab itself. The active tab is never folded away.

- **Opening** — double-click anything the editor can open in the Assets browser and it becomes a document tab: scenes, materials, particles, blueprints, and scripts / shaders / plain-text files (which open in the code editor). Double-clicking something that's **already open** doesn't open a second tab — it activates the one you have, with everything you'd left in it.
- **Switching** — clicking a tab brings up both the document *and* the workspace it belongs to. Click a material tab and you land in the Materials workspace with that material's graph loaded; click a second material tab and the graph swaps to it. A script or shader tab takes you to Scripting and focuses that file in the code editor; a scene tab takes you back to the Scene workspace with its scene, camera and selection as you left them.
- **…and the other way round** — switching workspace from the ribbon brings that workspace's document forward. Go to Materials and the material you were last editing is the active tab again; go back to Scene and so is the scene you came from. A workspace nothing maps to — Debug, Animation, Hub — leaves your tab selection alone.
- **New scene** — press `Ctrl+N` (or use the **`+`** button at the end of the tab strip). This opens a fresh **Untitled Scene** in its *own* new tab and switches to it; whatever you were working on stays open in its old tab, untouched. New scene never wipes the scene you're currently in.
- **Closing a tab** — only the **active** tab carries a **×**, so closing an inactive one means clicking it first. (Six close buttons across the strip was both noisy and a near-copy of the panel tab bar below it.) The last scene never shows one: at least one scene stays open, so Asset mode always has something to go back to. If the tab has **unsaved changes**, the editor brings it forward and asks first:
  - **Save & Close** — saves the scene (prompting for a location if it's never been saved), then closes the tab.
  - **Don't Save** — closes it and discards the edits.
  - **Cancel** (or `Escape`) — leaves the tab open.
  
  Closing a tab with no unsaved changes just closes it, with no prompt. The editor always keeps at least one scene tab open.
- **Reordering** — drag a tab sideways to move it. An accent bar shows where it will land, and the move is applied when you let go. Tabs that have folded into the caret menu can be dragged too: press a row in the menu and pull it out, and it lands where you drop it in the strip (something else folds away to make room). A plain click in the menu still just activates the tab.
- **Renaming** — double-click a tab to edit its name in place. `Enter` (or clicking away) commits, `Escape` cancels. For a saved document this **renames the file on disk**, keeping its extension and folder, and anything referencing the old path follows it — the same move the asset browser's rename performs. A brand-new unsaved tab has no file yet, so there it only changes the label. Renaming to a name that already exists in that folder is refused.

A tab with unsaved edits shows a **`*`** after its name, and the **save button** in the top bar turns **amber** — it's greyed out while the active tab is clean, so the color is the at-a-glance cue that the scene has work in it you haven't written to disk.

## Saving your scene

Press `Ctrl+S` to save. Scenes are stored as `.ron` files, kept by convention in your project's `scenes/` folder. You almost never edit these by hand — the editor writes them for you — but they are plain text if you ever want to peek.

### Scene thumbnails

Every save also takes a snapshot of the viewport and keeps it as the scene file's thumbnail, so the **Assets** browser shows you the level instead of a generic scene icon. Whatever the focused viewport was showing at the moment you pressed `Ctrl+S` is the picture you get — frame the shot you want, then save.

Thumbnails are always a 256×256 square, centre-cropped out of the viewport. Your dock layout decides the viewport's shape — widen the assets panel, open a second panel beside the viewport — and a thumbnail that inherited that shape would come out different every time you rearranged the editor. The centre of the view is what you framed, so that's what's kept; nothing is squashed to fit.

This is the only moment the picture is free to take. A material or a model thumbnail can be rendered on demand, but reproducing a scene's would mean loading the whole scene — the exact work the browser is there to save you. So a scene that has never been saved from the editor simply keeps its icon, and one you saved with the viewport panel closed keeps whatever thumbnail it already had.

Snapshots live in `<project>/.cache/thumbnails/scenes/`, alongside the texture, material and model thumbnail caches. That folder is disposable — delete it and everything else regenerates on demand, while scene thumbnails come back on the next save.

### Auto-save

The editor saves for you on a timer — **on by default**, every 5 minutes. Adjust it under **Settings → Auto-Save** (in the sidebar's **Editor** group): toggle it off, or change the interval (in seconds). In the last few seconds before each save the bottom-left status bar replaces **Ready** with a live **Auto save in Ns** countdown; when it reaches zero the scene is saved — through the exact same path as `Ctrl+S`, so a focused asset tab (a material, script, etc.) is never overwritten — and the label returns to **Ready**. Auto-save pauses while you're in Play mode.

Your project picks which scene loads first when the game runs. That's set in `project.toml` with a single `main_scene` line:

```toml
name = "My Game"
version = "0.1.0"
main_scene = "scenes/main.ron"
```

### Global scenes

You can also mark scenes **global**: they load *before* the main scene and stay alive the whole time, surviving every later scene change. Use one per concern — a HUD scene, a music scene, a networking scene — rather than duplicating that content in every level.

Set them in **Settings → Project → Global Scenes** (a toggle per scene in `scenes/`), which writes the `autoload` list:

```toml
main_scene = "scenes/main.ron"
autoload = ["scenes/ui.ron", "scenes/music.ron"]
```

Every entity a global scene spawns is marked persistent, so scene loads skip it — and scene *saves* skip it too, so it never gets baked into the level you happen to have open. Play and Simulate load them in the editor and Stop unloads them, so you can test without exporting.

This is also the only place a **loading screen** can work: everything in the outgoing scene is despawned partway through a scene change, so only a global scene's script is still running to show progress. See [Scripting → lifecycle hooks](/docs/r1-alpha7/scripting/lua#lifecycle-hooks) for `on_scene_loaded` and `scene_load_state()`.

### Your tabs come back

The document tabs across the top of the editor — scenes, plus any materials, scripts, shaders, particles, or blueprints you've opened — are remembered **per project**. Close the editor (or switch projects) and reopen, and the same tabs are restored in the same order, with the scene you were last looking at active. Only saved documents are remembered; a brand-new unsaved tab has no file to reopen, so it isn't restored. Restored scene tabs load lazily — clicking one loads its scene from disk the first time.

This is stored in `project.toml` (`editor_last_scene` and `editor_open_tabs`), so it travels with the project, and it's stripped from exported games.

> Good to know: only objects with a name are saved, and the editor leaves out runtime-only data (like rebuilt physics colliders and render handles) because the engine recreates it automatically when the scene loads. For the full technical breakdown, see [Project Structure](/docs/r1-alpha7/setup/project-structure) and [Components](/docs/r1-alpha7/engine-core/components).

## Reusing scenes inside scenes

You can drop one scene inside another as a **scene instance** — great for a prop, an enemy, or a room you want to reuse in many places. Edit the original once and every copy updates.

There are three ways to add one:

- **Drag** a scene file (`.bsn`) from the Asset Browser onto the **viewport** — it drops at the spot under your cursor.
- **Drag** the same file onto the **Hierarchy** — it's added at the scene root.
- Right-click in the Hierarchy and choose **Instance Scene…**, then pick a file.

Its contents appear nested under a new instance row. Choose **Unpack Scene Instance** if you'd rather break it apart into normal objects.

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
