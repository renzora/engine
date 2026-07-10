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

## The bottom panel

The strip of panels along the bottom of the Scene workspace — Assets, Hub Store, Console, Mixer, Sequencer, Timeline — is the **bottom panel**. By default it sits **under the viewport, not the full window width**, so the Hierarchy/Inspector column keeps its full height. It starts **closed** when the editor launches, keeping the viewport unobstructed:

- **Closed doesn't mean gone**: while closed, the panel collapses to just its **header strip** — a tab-bar-height row in the same place showing its tabs in a muted state. **Click any tab** to reopen the panel with that tab active.
- **`Ctrl+Space`** toggles it open and closed. Closing remembers everything — tab order, active tab, height, even splits you made inside it — and reopening restores all of it, **in the same place**: a strip docked under one column reopens under that column, a full-width one reopens full-width.
- **Chevron toggle**: the right end of the header carries a chevron in both states — **∨** on the open panel's tab bar collapses it, **∧** on the collapsed strip reopens it. The **∨** follows the strip wherever you dock it: any tab group holding Assets or Console below a horizontal divider gets one, not just a full-width bottom region.
- **Drag it closed**: pull the divider above the panel all the way down (past where it normally stops) and the panel snaps closed to the header strip. Reopening restores the height it had before the snap. This too works wherever the strip is docked.
- **Drag it open**: grab the collapsed strip's background and pull upward — the panel reopens and keeps sizing under your cursor as a live divider drag.
- **Drag any panel toward the bottom edge** of the window (a generous snap band along the bottom) and it snaps into a full-width bottom dock. This works for docked tabs and for floating windows dragged back over the editor.
- The toggle works per workspace: it collapses whatever is docked along the bottom of the current workspace's root — or, failing that, the region holding Assets/Console — so a bottom region you built yourself toggles too.

The open/closed state and the stashed panels persist in `~/.renzora/layout.json`, but every launch starts with the classic bottom strip closed.

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
