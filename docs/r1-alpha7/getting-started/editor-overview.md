# Editor Overview

Welcome to the Renzora editor! This is where you build your game — place objects, tweak them, write scripts, and press Play. This page is a quick visual tour so you feel at home the first time you open it.

Good news: the editor *is* the engine. What you see while editing is exactly how your game runs, so there are no surprises when you ship.

## The big picture

When you open a project, the editor fills the window with a few main areas.

![The full Renzora editor: the workspace ribbon runs across the top, the Scene tree sits on the left, the 3D viewport is in the middle with a colored move gizmo on a selected object, the Inspector is on the right, and the Assets browser runs along the bottom.](/assets/previews/interface.png)

From the screenshot above:

- **Top bar** — on the left, the **menu button** (☰) and everything that acts on the whole session: **Settings** (⚙), **undo / redo / save**, and the **Play** button. The **workspace ribbon** is in the center and the window buttons on the right. These sat in the viewport's own toolbar until recently; the top bar is on screen in every workspace, and none of them is a viewport action. Your open **document tabs** are not in the top bar itself — they're the strip directly under it, spanning the window (or, if you'd rather keep that row, a dropdown beside Play — see [Document tabs](/docs/r1-alpha7/editor/scenes#working-with-document-tabs)).
- **Toolbar** — the viewport's tools live on the viewport itself, in the strip along its top edge: Select / Move / Rotate / Scale, the snap steps, the shape and display menus, and the view-angle / World-Local controls. Undo, redo, save and **Play** are not here — they're session-wide, so they live in the top bar. Other panels keep their tools the same way, in their own header; the strip under the top bar is your open document tabs.
- **Left** — your **Scene** tree (everything in the current level), with the **Assets** browser — a file explorer for your project — below it.
- **Center** — the **3D viewport** where you see and move your world.
- **Right** — the **Inspector**, which shows the settings of whatever you click.
- **Bottom** — a collapsible strip under the viewport with the Console, Timeline, Mixer, and Shape Library (`Ctrl+Space` toggles it).

The window is borderless: drag the top bar to move it (double-click to maximize), and drag any edge to resize.

Click the **☰ menu button** to open the main menu. Your account is the first row — your username when you're signed in (hover it for **My Library** and **Sign Out**), or **Sign In** when you're not — followed by `File`, `Edit`, `View`, and `Help`. Hover one of those and its items slide out beside it, so everything that used to sit across the top bar is now one click away in a single dropdown. **Settings** is the last row — top-level, not buried in `File`; the gear button beside the hamburger opens the same panel in one click. **Notifications** moved in here too, under your username, now that the top bar has no bell.

The workspace ribbon has a **fixed width** so it can't be pushed around: add a tenth workspace and the ribbon stays exactly where it was. The document tabs on the row below get the window's full width instead. Either way, whatever no longer fits folds into a **caret button** (`⌄`) at the end of the strip, which opens a menu of the hidden tabs — click one to jump straight to it. The tab you're currently on never folds.

The **Help** submenu holds links to the documentation, YouTube, Discord, and the GitHub repo, plus **About Renzora Engine** — an overlay that shows the current version, a short description of the engine, and credits for the open-source community crates Renzora is built on. Each credit row links out to that project's repository; click anywhere outside the card (or press `Esc`) to close it.

## Workspaces

The tabs in the center of the top bar are **workspaces**. Each one is a ready-made layout tuned for a job, so the right tools are already in front of you:

- **Scene** — build and arrange your level (this is the default).
- **Scripting** — write Lua or Rhai code.
- **Blueprints** — visual node-based scripting, no typing required.
- **Animation** — clips, state machines, and timelines.
- **Materials** — design how surfaces look with a node graph.
- **Particles** — fire, smoke, sparkles, and other effects.
- **Debug** — performance and diagnostics while you test.
- **Hub** — the Marketplace, full screen: browse and install plugins, assets, and themes.

Click a tab to switch — and if you have more workspaces than the ribbon's width allows, the last ones fold into the caret menu at its end. You can drag tabs to reorder them — a blue insertion line shows where the tab will land as you drag — right-click to rename or remove, and press `+` to add a new one. Your changes to each layout — split sizes, where panels sit, which tab is active, even workspaces you add or rename — are saved automatically and restored the next time you open the editor. (The layout is stored per-user in `~/.renzora/layout.json`; delete that file to reset every workspace to its default.)

Two reset actions live under the **View** menu: **Reset Layout** restores the *active* workspace's panel arrangement to its built-in default, and **Reset Workspace** rebuilds the *entire* ribbon — discarding any workspaces you added, removed, renamed, or reordered and restoring every default workspace's layout.

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

Selecting an item here highlights it in the viewport and fills in the Inspector. See [Scenes & Hierarchy](/docs/r1-alpha7/editor/scenes) for more.

## The Inspector

When you select something, the **Inspector** shows all of its settings, grouped into sections called *components*.

![The Inspector showing the selected World Environment entity with its components: Name, Transform with Position/Rotation/Scale fields, Visibility, and a Directional Light with Illuminance, Color, and Shadows.](/assets/previews/inspector.png)

In the shot above you can see common components:

- **Transform** — position, rotation, and scale.
- **Visibility** — show or hide the object.
- **Directional Light** — brightness, color, and shadows.

Type new numbers into any field to change them live. Use **Add** at the top to attach more components (a physics body, a script, a custom one your plugins provide). For the full list of what each component does, see the [Inspector](/docs/r1-alpha7/editor/inspector) docs.

## The 3D viewport

The **viewport** is your window into the world. Click an object to select it, and a **gizmo** appears so you can move, rotate, or scale it by dragging the colored handles.

![The 3D viewport with a parked scooter selected in a street scene; a colored transform gizmo and selection outline let you move, rotate, or scale the object directly.](/assets/previews/viewport.png)

The toolbar at the top of the viewport switches your tool between **Select**, **Translate** (move), **Rotate**, and **Scale**. To look around, orbit, fly, and zoom with the mouse — the full controls are listed in [Viewport & Camera](/docs/r1-alpha7/editor/viewport). You can even open up to four viewports at once to see your scene from different angles.

## The Console

The **Console** is where the engine talks to you. Messages stream in as you work and while you test your game, sorted into categories so you can focus on what matters.

![The Console panel streaming categorized engine log messages, with Clear and Copy buttons, info/warning/error filters, a search box, and a command input that reads "Type /help for commands" at the bottom.](/assets/previews/console.png)

Use the filter buttons to show only warnings or errors, search to find a message, and the box at the bottom to type **slash commands** (start with `/help` to see what's available).

The Console keeps the most recent **100** messages by default and drops the oldest as new ones arrive — a deliberately small cap, because each retained message is a row the panel has to lay out, and a very long backlog can cost frames. Want deeper scrollback? Raise **Settings → General → Developer → Console Log Limit** (identical messages already collapse into one row with a count, so the limit measures distinct entries).

## The Assets browser

The **Assets** browser along the bottom is a file explorer for your project: a folder tree on the left and a grid (or list) of the current folder's files and sub-folders on the right.

What you can do here:

- **Double-click** a folder to open it, or a file to open it in its editor (materials, blueprints, scripts, particles, …). Double-clicking a **scene** (`.bsn`) opens it in its own scene tab, loaded from disk.
- **Click** to select; `Ctrl+Click` toggles, `Shift+Click` extends the range.
- **Left-drag** in empty space to box-select. Drag near the top or bottom edge and the grid **auto-scrolls** so the selection can reach files that are off-screen.
- **Drag** a file onto a folder to move it, or out into the viewport to spawn it. Dragging a **scene** (`.bsn`) onto the viewport or the Hierarchy adds it as a nested **scene instance**. While mid-drag, **hover over another panel's tab** for a moment and that tab springs to the front — so you can reveal a drop target (the Viewport, the Inspector, …) that's hidden behind another tab without ever letting go of the drag.
- **Right-click** any item for an **Open** action (routes to the matching editor; for a scene this is **Open Scene**, which loads it into its own tab), **Rename**, Duplicate, Favorite, Reveal in Explorer, Delete — and a color-coded **create-new** section (the same one the **Add** button opens) so you can make a new asset without reaching for the toolbar. New files land in the current folder.
- To rename a file or folder inline, press **`F2`**, **click its name** while it's already selected, or pick **Rename** from the right-click menu. The whole name starts highlighted, so typing (or `Delete`) replaces it; press `Enter` to confirm or `Esc` to cancel.
- Use the toolbar to **Add** a new asset, **Import**, create a **New Folder**, change the **sort** order, switch between **grid and list** views, and zoom the tiles. The **Add** menu (and the right-click menu) creates a **Material**, **Blueprint**, **Lua Script**, **Rhai Script**, **Particle**, **Template** (HTML markup UI), or **Scene (BSN)** — each row color-coded to match its file type's accent on the tiles.

## Settings

`Ctrl+,` (or the ⚙ button beside the hamburger) opens **Settings**: a search box and a category list on the left, the settings themselves on the right. A category is a *page* — it stacks one or more collapsible **sections**, so related settings stay together instead of each one costing you a trip back to the sidebar.

| Group | Category | Sections |
|---|---|---|
| **Project** | Project | Project, Global Scenes |
| | Window | Window, Render Resolution |
| | Rendering | 3D Rendering, 2D Rendering |
| **Appearance** | Interface | Fonts, Language, Display, Hierarchy, Inspector, UI Workspace (incl. Document Tabs) |
| | Theme | Active Theme, Semantic Colors, Surfaces, Text, Widgets, Panels, Syntax Tokens, Editor Chrome, Widget Styles |
| **Editor** | General | Developer, Renderer, Import |
| | Auto-Save | Auto-Save |
| | Viewport | Grid, Labels, Performance |
| | Camera | Camera |
| | Gizmos | Gizmos |
| | Scripting | Scripting, Code Editor |
| **Controls** | Input | Input actions and their bindings |
| | Shortcuts | One section per shortcut category |
| **Plugins** | *one per plugin* | Whatever the plugin registers |

Everything under **Project** is stored in the project's `project.toml` and travels with the project; everything else is per-user, in `~/.renzora/editor.toml`.

**Window vs Render Resolution** trips people up, because both have a width and a height. The **window** is the OS surface your shipped game opens — its size, whether it's resizable, and windowed / fullscreen / borderless. The **render resolution** is what the camera actually renders at before being scaled onto that window, and it only takes effect once **Stretch Mode** is set to *Viewport*. Leave Stretch Mode disabled and the two are the same thing. Turn it on and set the resolution to, say, 320×180, and you get chunky pixel-art upscaled to a 1080p window.

## Scaling the UI

If the editor looks too small (or too large) on your monitor, open **Settings → Interface** (`Ctrl+,`) and pick a **UI Scale** under *Display* — from 75% to 300%, applied instantly on top of your OS DPI setting. The choice is saved per user, so it sticks across projects and restarts. If you ever pick a scale that makes things awkward, press `Ctrl+0` to snap back to 100%.

## Handy shortcuts

A couple of shortcuts you'll use constantly:

- **Ctrl + P** — open the **command palette**, a quick search for any action or tool.
- **Ctrl + Z** / **Ctrl + Y** — undo and redo.

## What's next?

Now that you can find your way around, learn about [Core Concepts](/docs/r1-alpha7/getting-started/concepts) — entities, components, scenes, and how scripts attach to them.

Want to build your own editor panels or dig into the architecture? That's covered in the advanced [Building Editor Panels](/docs/r1-alpha7/editor-dev/panels) guide.
