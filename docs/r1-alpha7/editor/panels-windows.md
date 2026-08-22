# Panels & Windows

Every tool in the editor — the Viewport, Inspector, Hierarchy, Assets, Console, and the rest — lives in a **panel**. Panels sit in a dock: they stack as tabs, split the window into regions, and can be rearranged however you like. This page covers moving panels around, and pulling them out into **floating windows** so you can spread the editor across multiple monitors.

## Rearranging panels

Grab any panel's tab and drag it:

| Gesture | What it does |
|---------|--------------|
| **Click a tab** | Switch to that panel |
| **Drag a tab onto another panel's center** | Add it as a tab there |
| **Drag a tab onto a panel's edge** | Split — the panel takes that half |
| **Drag a tab to the dock's edge or corner** | Full-height column / full-width row across the whole workspace |
| **Drag a tab within its tab bar** | Reorder tabs |
| **Drag a tab onto the workspace ribbon** | Spawn a new workspace containing just that panel |

While dragging, a highlight previews exactly where the panel will land. The `+` button in any tab bar adds another panel to that group, and the `×` on a tab closes it. When a group holds more tabs than fit, the tabs **scroll horizontally** — hover the tabs and use the mouse wheel to slide the overflow into view. The scroll has no visible scrollbar, and the `+` button stays pinned to the right of the tabs so it's always reachable.

**Move a whole tab group at once**: every tab bar has a **grip handle at its far left** (⋮⋮). Drag it to move the entire group — all its tabs, keeping their order and active tab — to any drop target a single tab accepts: split against another panel, dock against a window edge, or merge into another group's tab bar. The drag ghost shows the active panel's name plus `+N` for the tabs riding along.

## Scrolling panels

Any scrollable panel (Hierarchy, Inspector, Assets, Console, …) accepts three gestures, always aimed at the panel under the cursor:

- **Mouse wheel** — the classic smooth scroll.
- **↑ / ↓ arrow keys** — hover the panel and hold an arrow key to scroll it, browser-style. Arrows stand down whenever something is using them as caret keys (a focused text field, the code editor, or a numeric field you're typing into).
- **Middle-click drag** — grab the content and pan it with the cursor; the grip holds even if the drag strays outside the panel. Views that scroll both axes (like the tileset atlas) pan on both.

All three honour **Settings → Interface → Display → Scroll Speed**, a multiplier on the scroll rate (1.5 is the default feel) persisted per user in `~/.renzora/editor.toml`.

## Narrow panels

Panel toolbars stay on one row as you drag a panel narrower. Buttons never squash or split their labels across two lines — instead the labels drop away and the buttons become icon-only keys, with the name moving to a hover tooltip. Flexible controls (search boxes, the Assets breadcrumb) shrink around them, down to a floor that keeps them usable.

- **Assets** — below roughly 820px the **Add**, **Import**, **New Folder** and **Sort** buttons go icon-only, the item count hides and the zoom slider slims, so the breadcrumb path keeps a readable share of the row. Narrower still (~310px) the panel drops the grid entirely and becomes a tree-only file browser; the toolbar goes with the grid, and its three actions fold into a single **+ Add** dropdown sat to the right of the tree's search box. Its menu carries **New Folder** and **Import** followed by the usual create-new list, so nothing is lost at any width and the actions cost no extra row.
- **Hierarchy** — below roughly 210px **+ Add Entity** collapses to a **+** key so the entity search keeps its width.

## The bottom panel

The full-width strip along the bottom of the editor — Console, Assets, Timeline, Mixer, Shape Library — is the **bottom panel**. It is **global**: one panel shared by every workspace, not a region inside any one of them. Switch from Scene to Blueprints to Animation and it stays exactly as you left it.

That is what makes it the place for panels you want everywhere. Dock the Asset browser here once and it is in every workspace, instead of adding a copy to each.

- **It is pinned.** You can't move the panel itself — it has no drag handle, and it always spans the bottom of the window above the status bar. Individual **tabs** still drag in and out freely, so you decide what lives in it.
- **It overlays, it doesn't squeeze.** Making it taller covers the panels above rather than compressing them, so your workspace's proportions are never disturbed by resizing it. Drag it back down and everything above is exactly where you left it.
- **Resize** by dragging its **top edge**, or the **empty space in its header** to the right of the tabs. Both show a ↕ cursor.
- **`Ctrl+Space`** toggles it open and closed.
- **Closed doesn't mean gone**: it collapses to its **header strip**, a tab-bar-height row just above the status bar showing its tabs muted. **Click any tab** to reopen with that tab active, at a quarter of the editor's height.
- **Chevron toggle** at the right end of the header in both states — **∨** collapses the open panel, **∧** reopens the collapsed strip.
- **Drag it open**: grab the collapsed strip's background and pull upward — it opens and keeps sizing under your cursor in one gesture. **Drag it closed** the same way: pull the top edge down past its minimum and it snaps shut.

Its contents, height and open/closed state persist in `~/.renzora/layout.json`, alongside — not inside — the workspace layouts.

> **Upgrading from an earlier version?** Layouts written before the bottom panel became global are migrated the first time you launch: every workspace's bottom strip, and anything left in a closed one, is folded into the single shared panel and de-duplicated. Nothing is lost, but your workspaces will no longer each carry their own copy of the Console.

## Floating windows

Three ways to undock a panel into its own floating window:

- **Ctrl + drag its tab** — the panel tears off into a window that follows your cursor; release to place it.
- **Press the grip** — hover a tab and a small handle appears at its left edge; press it to tear off that panel (no Ctrl needed).
- **Right-click its tab** — a menu offers **Undock** (the panel opens as a window under the cursor) and **Close panel** (removes the panel from its group).

Drop the window anywhere — including on another monitor. This is how you build a multi-monitor setup: keep the viewport maximized on one screen and float the Inspector, Console, or a second Viewport onto the others.

A floating window is a clean single-panel frame — a title bar and the panel's content, no tab strip:

- **Move it** by its title bar; **resize it** from any edge or corner.
- **Dock it back** by dragging it (title bar held) over the main window and releasing on a **tab bar** (joins that group as a tab) or on the **dock's edge/corner** (becomes a full column/row). A highlight previews the landing spot; release anywhere else and the window just stays where you left it.
- Floating windows layer like normal OS windows — arrange them freely across monitors. On the same monitor as the maximized editor, clicking the editor raises it over the float (alt-tab or the taskbar brings the float back).
- **Close** with the × in the title bar — the panel returns to the main dock, nothing is lost.

Your floating windows — panel, position, and size — are saved with the rest of the dock layout (`~/.renzora/layout.json`) and restored on the next launch, so a multi-monitor arrangement survives restarts.

> Tip: workspaces (the ribbon at the top) switch the **main window's** layout only. Floating windows stay put while you flip between workspaces, which makes them ideal for panels you always want visible — a Console, the Inspector, or a camera preview.

## A few notes

- A panel can only be in one place at a time: tearing it off or docking it back *moves* it.
- Each floating window hosts one panel. Want two panels on another monitor? Tear off two windows.
- Popup menus opened from panels in floating windows currently appear in the main editor window; tooltips follow the cursor into any window.
- If a floating window ends up stranded off-screen (say a monitor was unplugged), close and reopen the editor — or delete the `floating` section from `~/.renzora/layout.json` to reset just the floating windows.
