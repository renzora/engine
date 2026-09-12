# Undo and Redo

Every scene edit is undoable, and the history is shared across the panels that edit the same thing, so `Ctrl+Z` does what you expect no matter which panel you were working in.

| Key | Action |
|---|---|
| `Ctrl+Z` | Undo |
| `Ctrl+Y` or `Ctrl+Shift+Z` | Redo |

While you are typing in a text field, `Ctrl+Z` undoes the text edit rather than a scene action.

Undo is an editor feature. It does not ship in an exported game.

## What can be undone

| Area | Covered |
|---|---|
| Transforms | Moving, rotating and scaling in both the 3D and 2D viewports. Each drag is one step, and so is an arrow-key nudge. |
| The Inspector | Every field, plus adding, removing and disabling components. Removing restores a component with its edited values, not a default. |
| The Hierarchy | Rename, reparent, reorder, group, lock, hide, spawn and delete. |
| Deleting | Anything, including whole groups with their children. A deleted entity comes back under its original parent, in place. |
| Spawning | Shapes, presets, components. |
| Terrain | Each sculpt or paint stroke is one step. |
| Tilemaps | Each paint, erase or fill stroke, and the Randomise scatter. |
| The material editor | Node moves, connections, deletes, adds and pin edits. |
| The particle editor | Any change to the effect being edited. |
| The animation timeline | Keyframe drags, deletes, interpolation changes and recorded edits. |

## The History panel

Everything on the main scene appears in the **History** panel, which lists the stack and lets you jump to any point by clicking an entry.

<!-- screenshot: history.png - the History panel listing recent edits with the current position marked -->

## History is per document

The viewport, Hierarchy, Inspector and terrain tools all edit the same scene, so they share one history.

Self-contained document editors such as the material graph keep their own. Whichever editor you last clicked into is the one `Ctrl+Z` acts on. Edit some material nodes, press `Ctrl+Z`, and it undoes in the material editor. Click back into the viewport and `Ctrl+Z` undoes scene edits again.

Loading, creating or switching scenes clears the scene history, because the old entities no longer exist. Switching projects clears everything.

## See also

- [Keyboard Shortcuts](/docs/r1-alpha8/editor/shortcuts)
- [Making Edits Undoable](/docs/r1-alpha8/editor-dev/undo) for plugin authors
