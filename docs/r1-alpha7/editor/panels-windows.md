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

While dragging, a highlight previews exactly where the panel will land. The `+` button in any tab bar adds another panel to that group, and the `×` on a tab closes it.

## Floating windows

Three ways to undock a panel into its own floating window:

- **Ctrl + drag its tab** — the panel tears off into a window that follows your cursor; release to place it.
- **Press the grip** — hover a tab and a small handle appears at its left edge; press it to tear off that panel (no Ctrl needed).
- **Right-click its tab → Undock** — the panel opens as a window under the cursor.

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
