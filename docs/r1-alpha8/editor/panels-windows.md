# Panels and Workspaces

Every tool in the editor lives in a panel: the Viewport, Inspector, Hierarchy, Assets, Console and the rest. Panels stack as tabs, split the window into regions, and can be pulled out into their own windows.

## Workspaces

The tabs in the middle of the top bar are workspaces. Each one is a saved layout tuned for a job.

<!-- screenshot: workspace_ribbon.png - the workspace ribbon with Scene active -->

| Workspace | For |
|---|---|
| Scene | Building and arranging your level |
| UI | Laying out menus and HUDs |
| Scripting | Writing code |
| Blueprints | Node-based scripting |
| Animation | Clips, state machines and timelines |
| Materials | Designing surfaces with a node graph |
| Particles | Effects |

Click a tab to switch. Drag to reorder. Right-click to rename or remove. **+** adds one.

Your changes to each layout are saved automatically and restored next launch.

Drag a panel's tab onto the ribbon and drop it there to give that panel a whole workspace of its own.

### Resetting a layout

| Menu item | What it restores |
|---|---|
| View > Reset Layout | The active workspace's arrangement |
| View > Reset Workspace | The whole ribbon, discarding workspaces you added or renamed |
| View > Reset Global Docks | The bottom panel |

The first two never touch the bottom panel, and the third never touches a workspace.

## Adding and moving panels

<!-- screenshot: panel_add.png - the Add Panel picker with categories and a search box -->

Click **Add Panel** in any tab bar for a searchable picker with every panel grouped by category.

| Gesture | What it does |
|---|---|
| Click a tab | Switch to that panel |
| Drag a tab onto another panel's centre | Add it as a tab there |
| Drag a tab onto a panel's edge | Split, with the panel taking that half |
| Drag a tab to the dock's edge or corner | A full-height column or full-width row |
| Drag a tab within its tab bar | Reorder |
| Drag a tab onto the workspace ribbon | A new workspace holding just that panel |

A highlight previews where the panel will land.

<!-- screenshot: dock_drag.png - a panel mid-drag with the drop-zone highlight showing -->

Every tab bar has a grip handle at its far left. Drag that to move the whole group at once, keeping its tabs and their order.

When a group holds more tabs than fit, hover the tabs and scroll to slide the rest into view.

## Document tabs

The strip directly under the top bar is your open documents: one per scene, plus any materials, scripts, shaders or particles you have opened. Each is a chip with an icon in its type's colour and the document's name.

Clicking a tab brings up both the document and the workspace it belongs to. Click a material tab and you land in the Materials workspace with that material loaded. Switching workspace from the ribbon works the other way, bringing that workspace's document forward.

| | |
|---|---|
| Open | Double-click something in the Assets panel. Already open means it activates rather than opening twice. |
| New scene | `Ctrl+N`, or the **+** at the end of the strip. It opens in its own tab. |
| Close | The **×** on the active tab. The last scene keeps no ×. |
| Reorder | Drag sideways. |
| Rename | Double-click. For a saved document this renames the file on disk. |

A tab with unsaved edits shows a `*` after its name, and the save button in the top bar turns amber. Closing one with unsaved changes asks first, and so does closing the editor.

If you would rather not spend a row on the strip, **Settings > Interface > UI Workspace > Document Tabs** swaps it for a dropdown beside Play.

Your tabs are remembered per project and restored when you reopen it.

## The bottom panel

The full-width strip along the bottom ships holding **Assets**, **Timeline**, **Console**, **Mixer** and **Shape Library**.

It is global: one panel shared by every workspace, not a region inside any one of them. Switch from Scene to Materials and it stays exactly as you left it. That is what makes it the right place for panels you want everywhere.

<!-- screenshot: bottom_panel.png - the bottom panel open, showing its tabs and the set dropdown -->

| | |
|---|---|
| Toggle | `Ctrl+Space`, which always opens it at 40% of the editor height |
| Resize | Drag its top edge, or the empty space in its header |
| Collapse | The chevron at the right end of the header |
| Reopen at the height you left | The chevron again, or click a tab on the collapsed strip |

You cannot move the panel itself, but individual tabs drag in and out freely.

It gets out of the way while you drag an asset out of it, so you can see the viewport or the Inspector you are aiming at. Bring it back by taking the drag down near the bottom of the editor. What happens when you let go is a setting under **Settings > Interface > UI Workspace**.

### Panel sets

The dropdown at the right of the header names the set of tabs the panel is showing, starting as **Default**, and opens onto all of them.

One set per job is the point: a debugging set with the Console and a profiler, an authoring set with Assets and the Mixer, one click between them.

Switching keeps the set you are leaving exactly as it was. Drag rows in the menu to reorder, use the pencil to rename, **New Panel Set** to start an empty one, and **Remove This Set** to drop the one you are on.

### Overlay or Layout

The button left of the chevron switches how the panel takes its space.

**Overlay** floats the panel over the dock. Making it taller covers the panels above rather than squeezing them, so your workspace proportions are never disturbed. This is the default.

**Layout** snaps the panel into the bottom of the workspace and gives the panels above whatever height is left, the way a normal split does, so nothing is hidden underneath it.

Pick Overlay for a Console you pull up and dismiss. Pick Layout when the panel is part of how you work and you want the viewport to actually shrink for it.

Switching between them changes nothing about the panel's tabs, height or open state.

## Floating windows

<!-- screenshot: floating_window.png - a panel torn off into its own window beside the editor -->

Three ways to pull a panel out into its own window:

- `Ctrl` and drag its tab.
- Hover a tab and press the grip that appears at its left edge.
- Right-click the tab and choose **Undock**.

Drop it anywhere, including another monitor. This is how you build a multi-monitor setup: the viewport maximized on one screen, the Inspector or a second Viewport on the others.

Move a floating window by its title bar and resize it from any edge. To dock it back, drag it over the main window and release on a tab bar or on the dock's edge. The **×** in its title bar also returns the panel to the main dock.

Your floating windows are saved with the rest of the layout and restored next launch. Workspaces switch the main window's layout only, so floating windows stay put as you flip between them.

A panel can only be in one place at a time, and each floating window holds one panel. For two panels on another monitor, tear off two windows.

## Scrolling

Any scrollable panel takes three gestures, always aimed at the panel under your cursor: the **mouse wheel**, the **arrow keys** while hovering it, and a **middle-click drag** to pan the content.

Arrow keys stand down when something else is using them, such as a focused text field or the Hierarchy walking its own selection.

All three honour **Settings > Interface > Display > Scroll Speed**.

## Narrow panels

Drag a panel narrower and its toolbar stays on one row. Labels drop away, buttons become icon-only with the name in a tooltip, and search boxes shrink around them.

Narrow enough and the Assets panel drops its grid and becomes a tree-only file browser, with its actions folded into a single **+ Add** dropdown. Nothing is lost at any width.
