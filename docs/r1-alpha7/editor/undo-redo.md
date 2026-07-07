# Undo & Redo

Every scene edit you make in the editor is undoable, and the history is shared across the panels that edit the same thing — so `Ctrl+Z` does what you expect no matter which panel you were working in.

## The keys

| Key | Action |
|---|---|
| `Ctrl+Z` | Undo the last edit |
| `Ctrl+Y` *(or `Ctrl+Shift+Z`)* | Redo |

These are rebindable like every other shortcut (`Ctrl+P` → search "Undo", or Settings → Keybindings). Undo/redo is an **editor-only** feature — it does not ship in an exported game.

While you are typing in a text field, `Ctrl+Z` undoes the *text* edit, not a scene action — the scene undo only fires when a UI field doesn't have keyboard focus.

## What can be undone

Undo is wired through the whole editor:

- **Transforms** — moving, rotating, scaling with the gizmo, in **both 3D and 2D** viewports (each drag is one step; arrow-key nudges too).
- **The inspector** — every field: numbers, vectors, colors, toggles, text, dropdowns, asset drops and the per-field **reset** button. Scrubbing a value is a single undo step per drag; a separate drag on the same field is its own step. Also **adding/removing a component** and toggling a component **enabled/disabled** (removing restores the component with its edited values, not a default).
- **The hierarchy** — rename, reparent, reorder, group, lock, hide, spawn and delete.
- **Deleting anything** — lights, cameras, imported models, 2D sprites/nodes, and groups (with all their children and components) restore faithfully, not just primitive shapes.
- **Spawning** — shapes, presets, components, drawn meshes.
- **Terrain** — each sculpt or paint stroke is one undo step (this used to be a separate, hidden history; it is now part of the main one).
- **Tilemaps** — each paint/erase/fill stroke, and the Randomise scatter, is one step.
- **The material editor** — node moves, connections, deletes, adds, and pin edits (its own per-material history).
- **The blueprint editor** — node/connection/parameter edits (its own per-blueprint history).
- **The particle editor** — any change to the effect being edited.
- **The animation timeline** — keyframe drags, deletes, interpolation changes and recorded edits.

Anything on the main scene shows in the **History** panel, which lists the undo/redo stack and lets you jump to any point by clicking an entry.

## History is per-document

The viewport, hierarchy, inspector and terrain tools all edit the same scene, so they share **one** undo history — undoing in any of them walks the same timeline.

Self-contained document editors (the **material graph**, the **blueprint graph**) keep their **own** history. Whichever editor you last clicked into is the one `Ctrl+Z` acts on: edit some material nodes, press `Ctrl+Z`, and it undoes in the material editor; click back into the viewport and `Ctrl+Z` undoes scene edits again.

Loading, creating or switching scenes clears the scene history (the old entities no longer exist), and switching projects clears everything.

## See also

- [Keyboard Shortcuts](shortcuts.md) — the full key list and how to rebind.
- [Making edits undoable](../editor-dev/undo.md) — for plugin authors adding undo to a new panel.
