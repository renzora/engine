# Modeling & Sculpting

Renzora has a built-in mesh editor: press **Tab** with a mesh entity selected
and the viewport switches from Scene mode to **Edit mode**, where you work on
the mesh's vertices, edges, and faces directly — Blender-style. A separate
**Sculpt mode** deforms the surface with brushes. Any entity with a mesh can
be edited: primitives (cube, plane, sphere, …) and imported models alike.

Edits are saved with the scene. When a scene loads, edited geometry wins over
the original primitive or model source, so your changes survive reloads and
ship with the exported game.

## Entering and leaving Edit mode

- Select a mesh entity in the viewport or hierarchy, then press **Tab**
  (rebindable in *Settings → Shortcuts* under *Modeling*).
- Press **Tab** again to return to Scene mode. Edits bake back into the mesh
  automatically.
- The status bar shows *Edit Mode* / *Sculpt Mode* while active, and the
  viewport header's mode dropdown mirrors it.
- The **Modeling** panel (category *3D*) holds the tool buttons, settings,
  and a shortcut cheatsheet.

While in Edit mode, clicking empty space releases the current mesh so you can
click a different entity to edit it without leaving the mode.

## Selection

| Input | Action |
|---|---|
| `1` / `2` / `3` | Vertex / Edge / Face select mode (selection converts across modes) |
| Click | Select element under cursor |
| Shift+Click | Add / remove from selection |
| Alt+Click (edge mode) | Select the whole edge loop |
| `A` | Select all / deselect all |

## Modeling tools

| Input | Tool |
|---|---|
| `G` | Grab — move the selection on the view plane. Tap `X`/`Y`/`Z` to lock an axis (tap again to release). LMB commits, Esc/RMB cancels. |
| `E` | Extrude the selection (verts → wire, edges → quad strips, faces → region with side walls) and immediately grab it along the face normal. |
| `Ctrl+R` | Loop cut — a preview loop follows the edge ring under the cursor; scroll to add up to 16 cuts; LMB commits, Esc/RMB cancels. |
| `I` | Inset the selected faces (amount set in the Modeling panel). |
| `X` / `Del` | Delete the selection (verts cascade to faces; edges take their faces; faces go alone). |
| `Ctrl+X` | Dissolve — remove edges/verts while healing the surrounding faces. |
| `M` | Merge the selected verts at their center. |

Panel-only operations (Modeling panel → *Operations*):

- **Subdivide** — splits every selected face; triangles become 4 triangles,
  quads and n-gons become a fan of quads around a center vertex.
- **Merge by Distance** — welds all vertices closer than *Weld Dist*
  (remove doubles).
- **Bisect X/Y/Z** — cuts the whole mesh along the chosen local axis plane
  through the origin and selects the cut loop.
- **Mirror X/Y/Z** — symmetrize: keep the positive side, mirror it to the
  negative side, weld the seam.
- **Array** — duplicate the mesh *Array Count* times along *Array Offset*
  (relative to the mesh bounds, or absolute), welding touching copies.

### X Symmetry

Toggle **X Symmetry** in the Modeling panel and grab edits mirror onto the
matching vertices across the local X plane (the mesh must be symmetric for
partners to be found). The same toggle mirrors sculpt brushes.

### Join (Scene mode)

With several mesh entities selected in Scene mode, **Ctrl+J** joins them into
the first-selected entity: geometry is transformed into its local space and
appended, and the other entities are removed. Joining is not undoable.

## Sculpt mode

Pick **Sculpt** in the viewport header's mode dropdown (or the Modeling
panel). Tab exits back to Scene mode.

| Brush | Effect |
|---|---|
| **Draw** | Pushes the surface out along the average normal (Ctrl: in) |
| **Smooth** | Relaxes vertices toward their neighbours' average |
| **Grab** | Drags the region under the cursor rigidly with the mouse |
| **Inflate** | Moves each vertex along its own normal — puffs volume |
| **Flatten** | Pulls vertices onto the average plane under the brush |
| **Pinch** | Pulls vertices toward the brush center (Ctrl: pushes apart) |

| Input | Action |
|---|---|
| LMB drag | Apply brush stroke |
| `Ctrl` | Invert the brush |
| `Shift` | Temporary Smooth |
| `[` / `]` | Shrink / grow the brush radius |

Radius and strength are also on the Modeling panel. Normals recompute live
during the stroke, and each stroke is one undo step.

## Undo

Every modeling operation and every committed grab/stroke records to the
scene undo stack — `Ctrl+Z` / `Ctrl+Y` work as usual while editing.

## Limitations

- Meshes must be indexed triangle lists to enter Edit mode (all primitives
  and standard imports are). Coincident vertices are welded on entry and
  coplanar triangle pairs are shown as quads.
- Edits to the *children of glTF model instances* don't persist across scene
  loads — the model re-instantiates from its source file. Editing works, but
  save-persistence currently covers primitives, flattened imports, and joined
  meshes.
- Dissolve on faces, bevel, and a free-form knife are not implemented yet;
  Bisect covers planar cuts.
- Materials, UVs and normals are carried through edits; UVs of newly created
  geometry are interpolated from the source vertices, so heavily extended
  meshes may need external UV work.
