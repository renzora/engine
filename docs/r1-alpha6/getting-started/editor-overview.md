# Editor Overview

Welcome to the Renzora editor! This is where you build your game — place objects, tweak them, write scripts, and press Play. This page is a quick visual tour so you feel at home the first time you open it.

Good news: the editor *is* the engine. What you see while editing is exactly how your game runs, so there are no surprises when you ship.

## The big picture

When you open a project, the editor fills the window with a few main areas.

![The full Renzora editor: the workspace ribbon runs across the top, the Scene tree sits on the left, the 3D viewport is in the middle with a colored move gizmo on a selected object, the Inspector is on the right, and the Assets browser runs along the bottom.](/assets/previews/interface.png)

From the screenshot above:

- **Top bar** — menus (`File`, `Edit`, `View`, `Help`) on the left, the **workspace ribbon** in the center, and Play, Settings, and window buttons on the right.
- **Left** — your **Scene** tree (everything in the current level).
- **Center** — the **3D viewport** where you see and move your world.
- **Right** — the **Inspector**, which shows the settings of whatever you click.
- **Bottom** — the **Assets** browser, a file explorer for your project.

The window is borderless: drag the top bar to move it (double-click to maximize), and drag any edge to resize.

## Workspaces

The tabs in the center of the top bar are **workspaces**. Each one is a ready-made layout tuned for a job, so the right tools are already in front of you:

- **Scene** — build and arrange your level (this is the default).
- **Scripting** — write Lua or Rhai code.
- **Blueprints** — visual node-based scripting, no typing required.
- **Animation** — clips, state machines, and timelines.
- **Materials** — design how surfaces look with a node graph.
- **Particles** — fire, smoke, sparkles, and other effects.
- **Debug** — performance and diagnostics while you test.

Click a tab to switch. You can drag tabs to reorder them, right-click to rename or remove, and press `+` to add a new one. Your changes to each layout — split sizes, where panels sit, which tab is active, even workspaces you add or rename — are saved automatically and restored the next time you open the editor. (The layout is stored per-user in `~/.renzora/layout.json`; delete that file to reset every workspace to its default.)

## Panels can go anywhere

Every workspace is made of **panels** that you can rearrange to taste. Drag a panel's tab and drop it on the edge of another panel to split the space, or onto its center to stack it as a new tab. Drag the divider between panels to resize.

You can also **drag a panel's tab up onto the workspace ribbon** (the tabs in the top bar, or the `+`) and drop it there: that pops the panel out into a brand-new workspace of its own, named after the panel. It's the quickest way to give a single tool the whole screen — the panel moves out of its old workspace, and the new one is selected for you.

To add a panel, click **Add Panel** on an empty space. You'll get a searchable picker with everything grouped by category:

![The Add Panel picker, listing every panel grouped by category such as Scene, Material, Debug, Audio, and Shader, with a search box at the top.](/assets/previews/panels.png)

Don't worry about memorizing these — just open what you need, when you need it.

## The Scene hierarchy

The **Hierarchy** panel is the tree of everything in your scene: lights, cameras, models, terrain, and more. Items can be nested, so a model can contain its own parts.

![The Scene hierarchy panel showing a tree of entities — Terrain, World Environment, Camera, and an imported model with child parts — plus an Add Entity button, a search box, and per-row eye (visibility) and lock toggles.](/assets/previews/hierarchy.png)

What you can do here:

- Click **+ Add Entity** to create something new.
- **Drag** an item onto another to nest it (parent / child).
- **Right-click** for duplicate, delete, or rename.
- Click the **eye** to hide an item, or the **lock** to stop accidental edits.

Selecting an item here highlights it in the viewport and fills in the Inspector. See [Scenes & Hierarchy](/docs/r1-alpha5/editor/scenes) for more.

## The Inspector

When you select something, the **Inspector** shows all of its settings, grouped into sections called *components*.

![The Inspector showing the selected World Environment entity with its components: Name, Transform with Position/Rotation/Scale fields, Visibility, and a Directional Light with Illuminance, Color, and Shadows.](/assets/previews/inspector.png)

In the shot above you can see common components:

- **Transform** — position, rotation, and scale.
- **Visibility** — show or hide the object.
- **Directional Light** — brightness, color, and shadows.

Type new numbers into any field to change them live. Use **Add** at the top to attach more components (a physics body, a script, a custom one your plugins provide). For the full list of what each component does, see the [Inspector](/docs/r1-alpha5/editor/inspector) docs.

## The 3D viewport

The **viewport** is your window into the world. Click an object to select it, and a **gizmo** appears so you can move, rotate, or scale it by dragging the colored handles.

![The 3D viewport with a parked scooter selected in a street scene; a colored transform gizmo and selection outline let you move, rotate, or scale the object directly.](/assets/previews/viewport.png)

The toolbar at the top of the viewport switches your tool between **Select**, **Translate** (move), **Rotate**, and **Scale**. To look around, orbit, fly, and zoom with the mouse — the full controls are listed in [Viewport & Camera](/docs/r1-alpha5/editor/viewport). You can even open up to four viewports at once to see your scene from different angles.

## The Console

The **Console** is where the engine talks to you. Messages stream in as you work and while you test your game, sorted into categories so you can focus on what matters.

![The Console panel streaming categorized engine log messages, with Clear and Copy buttons, info/warning/error filters, a search box, and a command input that reads "Type /help for commands" at the bottom.](/assets/previews/console.png)

Use the filter buttons to show only warnings or errors, search to find a message, and the box at the bottom to type **slash commands** (start with `/help` to see what's available).

## The Assets browser

The **Assets** browser along the bottom is a file explorer for your project: a folder tree on the left and a grid (or list) of the current folder's files and sub-folders on the right.

What you can do here:

- **Double-click** a folder to open it, or a file to open it in its editor (materials, blueprints, scripts, particles, …).
- **Click** to select; `Ctrl+Click` toggles, `Shift+Click` extends the range.
- **Left-drag** in empty space to box-select. Drag near the top or bottom edge and the grid **auto-scrolls** so the selection can reach files that are off-screen.
- **Drag** a file onto a folder to move it, or out into the viewport to spawn it.
- **Right-click** any item for **Rename**, Duplicate, Favorite, Reveal in Explorer, Delete — and a color-coded **create-new** section (the same one the **Add** button opens) so you can make a new asset without reaching for the toolbar. New files land in the current folder.
- To rename a file or folder inline, press **`F2`**, **click its name** while it's already selected, or pick **Rename** from the right-click menu. The whole name starts highlighted, so typing (or `Delete`) replaces it; press `Enter` to confirm or `Esc` to cancel.
- Use the toolbar to **Add** a new asset, **Import**, create a **New Folder**, change the **sort** order, switch between **grid and list** views, and zoom the tiles. The **Add** menu (and the right-click menu) creates a **Material**, **Blueprint**, **Lua Script**, **Rhai Script**, **Particle**, **Template** (HTML markup UI), or **Scene (BSN)** — each row color-coded to match its file type's accent on the tiles.

## Scaling the UI

If the editor looks too small (or too large) on your monitor, open **Settings → Interface** (`Ctrl+,`) and pick a **UI Scale** under *Display* — from 75% to 300%, applied instantly on top of your OS DPI setting. The choice is saved per user, so it sticks across projects and restarts. If you ever pick a scale that makes things awkward, press `Ctrl+0` to snap back to 100%.

## Handy shortcuts

A couple of shortcuts you'll use constantly:

- **Ctrl + P** — open the **command palette**, a quick search for any action or tool.
- **Ctrl + Z** / **Ctrl + Y** — undo and redo.

## What's next?

Now that you can find your way around, learn about [Core Concepts](/docs/r1-alpha5/getting-started/concepts) — entities, components, scenes, and how scripts attach to them.

Want to build your own editor panels or dig into the architecture? That's covered in the advanced [Building Editor Panels](/docs/r1-alpha5/editor-dev/panels) guide.
