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
- **↑ / ↓ arrow keys** — hover the panel and hold an arrow key to scroll it, browser-style. Arrows stand down whenever something is using them as caret keys (a focused text field, the code editor, or a numeric field you're typing into), and whenever a focused list panel is walking its own selection with them — the [Hierarchy tree](/docs/r1-alpha8/editor/scenes#selecting-objects) does this, so clicking a row hands it the arrows until you click elsewhere.
- **Middle-click drag** — grab the content and pan it with the cursor; the grip holds even if the drag strays outside the panel. Views that scroll both axes (like the tileset atlas) pan on both.

All three honour **Settings → Interface → Display → Scroll Speed**, a multiplier on the scroll rate (1.5 is the default feel) persisted per user in `~/.renzora/editor.toml`.

## Narrow panels

Panel toolbars stay on one row as you drag a panel narrower. Buttons never squash or split their labels across two lines — instead the labels drop away and the buttons become icon-only keys, with the name moving to a hover tooltip. Flexible controls (search boxes, the Assets breadcrumb) shrink around them, down to a floor that keeps them usable.

- **Assets** — below roughly 820px the **Add**, **Import**, **New Folder** and **Sort** buttons go icon-only, the item count hides and the zoom slider slims, so the breadcrumb path keeps a readable share of the row. Narrower still (~310px) the panel drops the grid entirely and becomes a tree-only file browser; the toolbar goes with the grid, and its three actions fold into a single **+ Add** dropdown sat to the right of the tree's search box. Its menu carries **New Folder**, **Import Files…** and **Import Folder…** followed by the usual create-new list, so nothing is lost at any width and the actions cost no extra row.
- **Hierarchy** — below roughly 210px **+ Add Entity** collapses to a **+** key so the entity search keeps its width.

## The bottom panel

The full-width strip along the bottom of the editor is the **bottom panel**. It ships holding **Assets, Timeline, Console, Mixer and Shape Library**, and it is **global**: one panel shared by every workspace, not a region inside any one of them. Switch from Scene to Blueprints to Animation and it stays exactly as you left it.

That is what makes it the place for panels you want everywhere. The Asset browser is docked here once rather than copied into each workspace — which is also why you won't find it in the Scene or Scripting layouts any more. Two copies of a panel are two independent panels, each with its own state, and a global one you can reach from anywhere is worth more than a per-workspace one you have to keep in sync.

Because it belongs to the editor and not to a workspace, **Reset Layout** and **Reset Workspace** leave it alone entirely. **View → Reset Global Docks** is the one action that restores it: a single set named **Default** holding the five tabs above, opened at its default height. It also discards any extra [panel sets](#panel-sets), so it is a full reset of the panel rather than of the set you happen to be on.

- **It is pinned.** You can't move the panel itself — it has no drag handle, and it always spans the bottom of the window above the status bar. Individual **tabs** still drag in and out freely, so you decide what lives in it.
- **Resize** by dragging its **top edge**, or the **empty space in its header** to the right of the tabs. Both show a ↕ cursor. It goes **all the way up to the top bar** — a full-height Assets browser or Console is one drag away, and the mode and chevron buttons ride along at the panel's top edge, so you can always put it back (`Ctrl+Space` closes it from anywhere too). In **Layout** mode the panel [hands itself over to Overlay](#overlay-or-layout) once it gets that tall.
- **It opens at 40% at most.** However tall you left it, the editor starts the next session with the panel capped at **40% of the dock region** — so a full-height Assets browser you pulled up yesterday isn't what you open onto today. A shorter height is restored exactly as you left it, and the cap applies only at load: drag it straight back up to the top bar if that's where you want it.
- **`Ctrl+Space`** toggles it open and closed. Opening this way always gives it **40% of the editor's height**, not whatever height it had when you last closed it — so the shortcut is a reliable "show me the panel" rather than something that occasionally reopens a sliver. Use the chevron when you want the height you left it at.
- **Closed doesn't mean gone**: it collapses to its **header strip**, a tab-bar-height row just above the status bar showing its tabs muted. **Click any tab** to reopen with that tab active, at the same 40%.
- **Chevron toggle** at the right end of the header in both states — **∨** collapses the open panel, **∧** reopens the collapsed strip.
- **Drag it open**: grab the collapsed strip's background and pull upward — it opens and keeps sizing under your cursor in one gesture. **Drag it closed** the same way: pull the top edge down past its minimum and it snaps shut.
- **It slides.** However you open or close it — the shortcut, either chevron, a tab on the collapsed strip — the panel travels in and out over about a sixth of a second rather than appearing and disappearing, so you can see where a panel covering 40% of the editor went. In **Overlay** it slides up from the bottom edge at its full size; in **Layout** it opens as an accordion, because there the height it takes is height the workspace above is giving up. **Dragging it yourself doesn't animate**: the top edge tracks your cursor, and the snap-shut at the bottom of that drag is instant — the panel is already at a sliver and already where you put it, so there is nothing left to play.
- **It gets out of the way while you drag an asset out of it.** Drag a file out of the Assets tab (or a shape out of the Shape Library) and the panel slides closed the moment your cursor leaves its top edge, so you can see the viewport, the hierarchy or the inspector slot you're aiming at. Dragging *within* the panel — onto a folder, onto another tab — never triggers it. To bring it back, take the drag **down near the bottom of the editor**, to the collapsed strip or the last inch or so above it. It deliberately isn't the space the panel would occupy: while the panel is closed that band is a full-width slice of somebody else's panel, and reopening on the way past would drop the panel on top of the target you were heading for. Whatever you do with the drag, the panel returns to the state it was in when the drag started, and a drag that begins with the panel already closed never opens it.

### Panel sets

The **dropdown left of the Overlay/Layout button** names the set of tabs the panel is currently showing — **Default** to start with — and opens onto all of them. It opens **downward into the panel** when the panel is tall enough to show the whole list, and **upward over the workspace** when it isn't, so the list is never sliced off at the status bar or at the top bar:

- **Pick a set** to switch to it. The one you're leaving keeps its tabs, its active tab and its splits, so switching back and forth costs nothing.
- **Reorder** them by dragging a row up or down the list. An accent bar shows where it will land, and the move is applied when you let go — the menu stays open, so putting three sets in the order you want is one trip rather than three. A plain click still picks the set; the drag only starts once you've moved a little, so the two never collide.
- **Rename** one with the **pencil** at its right-hand end — the row turns into a text field with the caret already in it. `Enter` (or clicking away) commits, `Escape` cancels, and an empty name cancels rather than leaving a row you can't read. You can rename any set from here, not just the one you're on.
- **New Panel Set** starts an empty one and opens the panel on it. Empty means the panel shows its **Add Panel** button — pick what goes in from there, or drag tabs in as usual.
- **Remove This Set** drops the set you're on and lands you on its neighbour. It only appears while you have more than one; the panel always keeps at least one set to put tabs in.

One set per *job* is the point: a debugging set with Console and the profiler, an authoring set with Assets and the Mixer, and one click between them rather than rebuilding the panel tab by tab. Sets belong to the panel, not to a workspace — like the panel itself, they're the same in every workspace — and they persist in `~/.renzora/layout.json`.

**An empty panel stays put — open or closed.** Close its last tab and the bar remains, with the set dropdown, the mode button and the **Add Panel** button still there. Collapse it from there and the header strip remains too, showing the set's name and an **empty** marker beside its **∧** chevron, so you can always open it again and add a panel back. (Both used to vanish, taking their own controls with them; `Ctrl+Space` still worked, but nothing on screen said so.)

### Overlay or Layout

The button immediately **left of the chevron** switches how the panel takes up its space. The icon shows the mode it is in now; click it to swap.

- **Overlay** (**▤ stack**, the default) — the panel floats over the dock. Making it taller covers the panels above rather than compressing them, so your workspace's proportions are never disturbed by resizing it. Drag it back down and everything above is exactly where you left it.
- **Layout** (**▤ rows**) — the panel snaps into the bottom of the workspace and the panels above are given the height that's left. Resizing it now reflows everything above, the way a normal dock split does, so nothing is ever hidden underneath it.

Pick Overlay when you want a Console you can pull up over your work and dismiss; pick Layout when the panel is part of how you work and you want the viewport to actually shrink to make room for it.

Both modes put the panel in the same place at the same height, and both are resized and toggled identically — only what happens to the workspace above differs. Switching is non-destructive: the panel's tabs, height and open state are untouched.

**Layout mode gives way near the top.** The panel resizes up to the top bar in either mode, but there is a point past which "the panels above are given the height that's left" stops meaning anything — there is no height left. Once a Layout panel is tall enough that less than ~120px of workspace would remain, it **switches to Overlay** for as long as it stays that tall, so the panels above keep their real size under it instead of being crushed into a stack of tab bars. The mode button follows: it shows **▤ stack** with a tooltip saying the panel is too tall to dock. Drag back down and Layout resumes on its own — nothing was saved, so your choice of mode survives the trip. Clicking the button while it's up there is read as "dock it properly": the panel drops to the tallest height Layout can actually hold.

Its sets, height, open/closed state and mode persist in `~/.renzora/layout.json`, alongside — not inside — the workspace layouts.

> **Upgrading from an earlier version?** Layouts written before the bottom panel became global are migrated the first time you launch: every workspace's bottom strip, and anything left in a closed one, is folded into the single shared panel and de-duplicated. Nothing is lost, but your workspaces will no longer each carry their own copy of the Console. The default tab set above applies to a **fresh** install only — an existing layout is migrated, never replaced, so a panel you had arranged is not swapped out for the shipped one. **Reset Global Docks** is how you ask for that deliberately.

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
