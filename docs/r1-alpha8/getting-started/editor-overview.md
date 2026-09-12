# Editor Tour

A quick walk around the editor so you know what you are looking at. Each area has its own page with the detail.

The editor is the engine. What you see while editing is how your game runs.

![The full Renzora editor: the workspace ribbon across the top, the scene tree on the left, the 3D viewport in the middle with a move gizmo on a selected object, the Inspector on the right, and the Assets browser along the bottom.](/assets/previews/interface.png)

## The five areas

| Area | What it is |
|---|---|
| Top bar | The menu button, Settings, undo, redo, save, Play, and the workspace ribbon. Anything that acts on the whole session. |
| Viewport | Your live 3D view, in the middle. Its own tools sit in a strip along its top edge. |
| Hierarchy | The tree of everything in the current scene. |
| Inspector | The settings of whatever you have selected. |
| Bottom panel | A collapsible strip shared by every workspace, holding Assets, Timeline, Console and the Mixer. `Ctrl+Space` toggles it. |

The window is borderless. Drag the top bar to move it, double-click to maximize, drag any edge to resize.

<!-- screenshot: top_bar.png - the top bar close up: menu button, settings gear, undo/redo/save, play controls, workspace ribbon, window buttons -->

## The menu

Click **☰** to open the main menu. Your account is the first row, then `File`, `Edit`, `View` and `Help`. Hover one and its items slide out beside it. **Settings** is the last row, and the gear button beside **☰** opens the same thing in one click.

`File` starts with **New Project**, **Open Project** and **Recent Projects**, which lists the last ten projects you opened. Click one to switch straight to it.

Leaving a project closes every document in it, so all three of those ask first when anything is unsaved.

<!-- screenshot: menu_file.png - the File menu open with the Recent Projects submenu showing -->

## Workspaces

The tabs in the middle of the top bar are workspaces. Each one is a layout tuned for a job.

| Workspace | For |
|---|---|
| Scene | Building and arranging your level. This is the default. |
| UI | Laying out menus and HUDs. |
| Scripting | Writing code. |
| Blueprints | Node-based scripting. |
| Animation | Clips, state machines and timelines. |
| Materials | Designing surfaces with a node graph. |
| Particles | Fire, smoke, sparks and other effects. |

Click a tab to switch. Drag tabs to reorder, right-click to rename or remove, and press **+** to add one. Your changes to each layout are saved automatically.

<!-- screenshot: workspace_ribbon.png - the workspace ribbon with Scene active -->

Full detail is in [Panels and Workspaces](/docs/r1-alpha8/editor/panels-windows).

## The Hierarchy

Everything in your scene, as a tree. Lights, cameras, models, terrain. Items nest, so a model can contain its own parts.

![The Hierarchy panel showing a scene tree with Terrain, World Environment, Camera and an imported model expanded into its parts, plus an Add Entity button, a search box, and per-row eye and lock toggles.](/assets/previews/hierarchy.png)

Click **+ Add Entity** to create something. Drag an item onto another to nest it. Right-click for duplicate, delete or rename. Click the eye to hide an item, or the lock to stop accidental edits.

Selecting something here highlights it in the viewport and fills in the Inspector. See [Scenes and Hierarchy](/docs/r1-alpha8/editor/scenes).

## The Inspector

Select something and the Inspector shows its settings, grouped into components.

![The Inspector showing a selected entity with its Name, Transform, Visibility and Directional Light components.](/assets/previews/inspector.png)

The row at the top is the entity header: its icon, its ID, its label colour, an eye to hide it and a lock to pin the Inspector to it. Below that comes one section per component. Type into any field to change it live.

Use **Add** at the top to attach more components. See [Inspector](/docs/r1-alpha8/editor/inspector).

## The viewport

Click an object to select it. A gizmo appears so you can move, rotate or scale it by dragging the coloured handles.

![The 3D viewport with a scooter selected in a street scene, a coloured transform gizmo attached to it.](/assets/previews/viewport.png)

The strip along the top switches your tool between Select, Move, Rotate and Scale, and holds the snap, shading and view-angle controls. You can open up to four viewports at once.

See [Viewport and Camera](/docs/r1-alpha8/editor/viewport).

## The Console

Where the engine talks to you. Messages stream in as you work and while you test.

![The Console panel streaming engine log messages, with Clear and Copy buttons, info, warning and error filters, a search box, and a command input at the bottom.](/assets/previews/console.png)

Filter to warnings or errors, search to find a message, and type slash commands in the box at the bottom. Start with `/help`.

The Console keeps the most recent 100 messages and drops the oldest. Identical messages collapse into one row with a count. Raise the limit under **Settings > Editor > Console Log Limit** if you need deeper scrollback.

## Two shortcuts to learn first

| | |
|---|---|
| `Ctrl+P` | The command palette. Search for any action or tool. |
| `Ctrl+Z` / `Ctrl+Y` | Undo and redo. |

The rest are in [Keyboard Shortcuts](/docs/r1-alpha8/editor/shortcuts).

## What's next

- [Core Concepts](/docs/r1-alpha8/getting-started/concepts) covers entities, components and scenes.
- [Your First Project](/docs/r1-alpha8/getting-started/first-project) builds something.
