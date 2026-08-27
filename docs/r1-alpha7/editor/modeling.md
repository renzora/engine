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

### Persistent editable topology

Edits are stored as a flat triangle list when you leave Edit mode (or
on every dirty live-bake, with the `EditedMeshApplied` marker
suppressing the scene-load rehydrator while the mesh is being
edited in place). The triangle list is enough to redraw the mesh,
but it can't express the *bounded faces* the editor uses to pick,
extrude, and loop-cut. A flat triangle list also can't tell two
coplanar extruded quads apart from one quad split across the
extrusion boundary.

Renzora solves this by storing the editor's bounded-face topology
alongside the triangle list on the same `EditedMesh` component:

- `face_vertices: Vec<u32>` — flattened vertex IDs for every
  editable face, in perimeter order. Each face contributes
  `face_vertex_counts[i]` consecutive entries here.
- `face_vertex_counts: Vec<u32>` — number of vertices per face.

When you re-enter Edit mode, the picker rebuilds `EditMesh.faces`
from these two arrays exactly, then re-derives the edge topology. No
triangle-guessing, no diagonal-boundary artefacts. Both fields are
`#[serde(default)]`, so scene files written before this change still
load through the import heuristic (`from_mesh` + coplanar-triangle
merge) — they just lose the explicit bounded-face contract.

If the topology is malformed (length mismatch, out-of-range vertex
ID, fewer than 3 vertices per face), the validator refuses it and
the system falls back to the import heuristic with a warning. The
fallback is hermetic — it never panics, never silently ships a
broken face layout.

This is the same model Blender has used for years: the render-time
triangle list is separate from the editable quad / n-gon face
boundaries, and the editor re-loads the latter verbatim on every
entry. Re-entry is also what makes multiple *Edit → bake → re-Edit*
cycles idempotent — each round of edits preserves the user's
extruded quads as quads, not as triangles the next guess might
mis-pair.

## Selection

| Input | Action |
|---|---|
| `1` / `2` / `3` | Vertex / Edge / Face select mode (selection converts across modes) |
| Click | Select element under cursor |
| Shift+Click | Add / remove from selection |
| Alt+Click (edge mode) | Select the whole edge loop |
| Click-drag on empty space | Marquee drag-select — drag a rectangle on empty space; elements inside it are selected on release (Shift promotes the rect to additive toggle) |
| `A` | Select all / deselect all |

### Mesh-edit overlay (Settings → Viewport → Mesh Edit)

Three controls tune how Edit-mode selection looks and behaves:

- **Vertex Size** — pixel side length of every unselected vertex dot.
  Range 1–12 (default 3). Dots are screen-space squares, so the size
  is constant regardless of camera zoom or distance.
- **Vertex Size (Selected)** — pixel side length of selected vertex
  dots. Range 1–12 (default 5). Bigger than the unselected size so the
  selection state reads at a glance, the way Blender's
  `bTheme::space_view3d.vertex_size` works.
- **X-Ray Select** — when off (default), Edit-mode vertex picking does
  a depth test: only the vertex closest to the camera is selectable.
  This stops a click near a sphere's silhouette from accidentally
  selecting the back-side vertex that happens to project onto the same
  screen point. Toggle on to allow through-selection (picks the
  closest vertex in *screen* space within the 8 px pick radius, so the
  back-side vertex can win when its projected position is closer to
  the cursor than the front-side vertex's).

### Face highlight (selected faces, Edit → Face mode)

Selected faces in Face mode get a translucent tinted fill plus a sharp
perimeter outline, the Blender-style `face_select` overlay look:

- **Fill** — a real 3D mesh triangulated the same way as
  `EditMesh::bake_to_mesh`: an `(n - 2)` triangle fan anchored at the
  face's first perimeter vertex, with the remaining vertices in
  `face.verts` order. Parenting to the edit target lets the overlay
  inherit the entity's transform. Drawn with a translucent `unlit`
  `StandardMaterial` (`srgba(1.0, 0.55, 0.1, 0.45)`).
  `depth_bias: 1.0` pulls the overlay in front of the cube's geometry
  as a small additional safeguard — in Bevy 0.19 positive `depth_bias`
  values render closer to the camera. **The triangulation match is the
  primary fix for the fan-shaped flicker:** the overlay and the
  underlying mesh rasterize the same surface into the same triangles,
  so the GPU's per-pixel depth interpolation doesn't disagree between
  them.
- **Outline** — a per-edge `gizmos.line` at full alpha on top of the
  fill. Reads as a crisp boundary against the translucent interior.

Clicking a face selects exactly the bounded `Face` the ray hits —
not every coplanar neighbour. Imported triangle pairs are already
merged into quads at bake (`merge_coplanar_triangle_pairs`), and an
edge between two coplanar faces (e.g. a freshly-extruded cube's top
and bottom quad) is a real topological boundary that Blender treats as
a face-to-face edge. Selection respects that boundary.

### Gizmo Thickness (Settings → Viewport)

Multiplier on the line width of every transform-gizmo line — translate
arrows, rotate rings, scale cubes, plus the plane-drag squares and
selection labels. Range 0.5–2.5, default 1.0 (unchanged). Live-applied
each frame, so the slider takes effect immediately on the gizmo
without restarting the editor.

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
