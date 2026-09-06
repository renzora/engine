# Modeling & Sculpting

> Modeling ships as the **Mesh Edit** native plugin, in `plugins/mesh_edit`. It
> is installed by default; if the Inspector has no Mesh Edit section and **Tab**
> does nothing, that plugin is disabled or absent (*Settings → Editor →
> Plugins*).
> Being a plugin is why it can be removed from a build that does not need it,
> and why its source sits beside the editor rather than inside the binary.

Renzora has a built-in mesh editor: press **Tab** with a mesh entity selected
and the viewport switches from Scene mode to **Edit mode**, where you work on
the mesh's vertices, edges, and faces directly. A separate
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
- The **Mesh Edit** section of the Inspector holds the tool buttons, settings
  and a shortcut cheatsheet. It is a component on the entity being edited, not
  a panel of its own: which object you are modeling is a property of that
  object, and it belongs where the rest of its properties are.

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

Each mode marks what it can pick: a dot on every vertex, a dot at every edge's
midpoint, and an inset outline on a selected face. Unpicked elements are faint —
they are a hint about where the elements are, not the thing you are looking at —
and the markers stay a constant size on screen however far away the mesh is.

## Handles

Edit mode's handles are for the operations a modeling selection actually has:

- **Extrude** — the stalk running out along the selection's normal, with a `+`
  at its tip. Drag it and new geometry pulls out along the normal.
- **Inset** — the ring at the stalk's base (faces only). Drag toward the centre
  of the face to inset further; drag back to the rim to undo it. The amount
  follows your cursor's distance from the centre, so the whole range is inside
  the ring however big it is drawn.

Both re-run their operator from the mesh as it was when you pressed, rather than
accumulating, so dragging back to where you started gives you exactly the mesh
you started with, and the whole gesture is one undo step.

**Selecting still works while they are up.** A click more than a handle's width
away is not a handle click and goes straight through to picking, and the handles
are drawn off the surface so the geometry under them stays visible.

### Transform handles

Move / Rotate / Scale on the selection are available too, from the **Gizmo**
button in the Inspector's Mesh Edit section (Move → Rotate → Scale → off). They
pivot on the **median of the selected vertices** rather than the object's
origin, so rotating three vertices at a far corner turns them about themselves.

They start **off**, because axis arrows offer nothing a face selection usually
wants while covering the geometry you are trying to click. Turn them on for the
times you do want to slide a selection along an axis.

Neither appears in Sculpt mode: a sculpt is a close read of a silhouette and
handles sit on the form you are judging, which is the same reason Sculpt hides
the selection box and collider cages.

## Modeling tools

| Input | Tool |
|---|---|
| `G` | Grab — move the selection on the view plane. Tap `X`/`Y`/`Z` to lock an axis (tap again to release). LMB commits, Esc/RMB cancels. |
| `E` | Extrude the selection (verts → wire, edges → quad strips, faces → region with side walls) and immediately grab it along the face normal. |
| `Ctrl+R` | Loop cut — a preview loop follows the edge ring under the cursor; scroll to add up to 16 cuts; LMB commits, Esc/RMB cancels. |
| `I` | Inset the selected faces (amount set in the Inspector). |
| `X` / `Del` | Delete the selection (verts cascade to faces; edges take their faces; faces go alone). |
| `Ctrl+X` | Dissolve — remove edges/verts while healing the surrounding faces. |
| `M` | Merge the selected verts at their center. |

Button-only operations (Inspector → Mesh Edit → *Operations*):

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

Toggle **X Symmetry** in the Inspector and grab edits mirror onto the
matching vertices across the local X plane (the mesh must be symmetric for
partners to be found). The same toggle mirrors sculpt brushes.

### Join (Scene mode)

With several mesh entities selected in Scene mode, **Ctrl+J** joins them into
the first-selected entity: geometry is transformed into its local space and
appended, and the other entities are removed. Joining is not undoable.

## Sculpt mode

Pick **Sculpt** in the viewport header's mode dropdown, or from the Mesh Edit
section of the Inspector. Tab exits back to Scene mode.

Entering Sculpt hides the selection box and the collider wireframes, and puts
both back on the way out. A sculpt is a close read of a silhouette, and those
two gizmos sit directly on the form you are judging.

| Brush | Effect |
|---|---|
| **Draw** | Pushes the surface out along the average normal (Ctrl: in) |
| **Clay** | Builds up in flat layers: mass, without a field of bumps |
| **Crease** | Pinches toward a centre line while pressing in — a narrow furrow, for wrinkles and seams |
| **Scrape** | Cuts back only what stands above the local plane, leaving what is below |
| **Smooth** | Relaxes vertices toward their neighbours' average |
| **Mask** | Paints where the surface must hold still (Ctrl erases) |
| **Grab** | Drags the region under the cursor rigidly with the mouse |
| **Snake Hook** | Pulls a limb out as a tube rather than a spike |
| **Inflate** | Moves each vertex along its own normal — puffs volume |
| **Flatten** | Pulls vertices onto the average plane under the brush |
| **Pinch** | Pulls vertices toward the brush center (Ctrl: pushes apart) |

| Input | Action |
|---|---|
| LMB drag | Apply brush stroke |
| `Ctrl` | Invert the brush |
| `Shift` | Temporary Smooth |
| `[` / `]` | Shrink / grow the brush radius |
| `Ctrl+I` | Invert the mask |
| `Alt+M` | Clear the mask |

Radius, strength and *Dyntopo Detail* are in the Inspector's Mesh Edit section.
Normals recompute live during the stroke, and each stroke is one undo step.

### Grab vs Snake Hook

Both drag the surface in the direction of the gesture, and the difference is
what happens when you keep going. Grab's falloff moves the centre of the brush
by the full distance and its rim by nothing, so a long pull shears the same
skirt of triangles further and further and the result comes to a point. Snake
Hook moves the inner core of the brush rigidly and falls off only across the
outer band, so the cross-section under the core is *carried* forward and arrives
intact — which is what pulling an arm out of a sphere needs.

Use Grab to move a form you already have, and Snake Hook to grow a new one.

### Masking

Masked geometry holds still under **every** brush. That is what lets you work on
a limb without the dab bleeding into the torso it grows out of, or refine a face
without flattening the ear beside it — the commonest reason a sculpt goes soft
is a brush quietly dragging the neighbouring form along with it.

Paint it with the **Mask** brush (Ctrl to erase), or use the buttons under
*Mask* in the Inspector:

- **Clear** (`Alt+M`) — unmask everything.
- **Invert** (`Ctrl+I`) — swap protected for free. Paired with a quick paint,
  this is the fastest way to isolate a region: mask the part you want to work
  on, invert, sculpt.
- **Smooth** — blur the mask against its neighbours. A hard mask boundary shows
  up as a hard *step* in the surface, because the brush moves one vertex fully
  and the one beside it not at all. Smooth the mask and the transition goes with
  it.

The mask is drawn as an outline around the protected region rather than a tint
over it, so it does not sit between you and the form you are judging.

Partial values work: a vertex at 0.5 moves half as far, which is what makes a
soft boundary soft. The mask rides on the vertex, so it survives dyntopo — a
split interpolates it, a collapse averages it — and it is a working state, not
something saved with the mesh.

### Dyntopo

**Dyntopo Detail** adds geometry under the brush as you work, and removes it
where the surface has bunched up. Set as a fraction of the brush radius: at
`0.25` a dab has roughly four triangles across it and can only make a bump; at
`0.05` it has twenty and can make a fold. `0` turns it off, which is the
default — dyntopo rewrites topology on every dab, and that is not something to
start doing to an authored mesh without being asked.

Without it, every brush is limited to the vertices that are already there, so a
long pull stretches a handful of triangles into a spike no matter which brush
made it. With it, the footprint gains geometry as it deforms.

Two details worth knowing:

- Refinement runs **before** each dab, not after. A surface that has already
  been stretched has nowhere left to put the detail.
- A stroke with dyntopo on records its undo step as a whole-mesh snapshot rather
  than a vertex-delta list, because the vertex numbering does not survive the
  decimation pass. It costs more memory per stroke and is the only thing that
  makes undo correct there.

## Driving the tools from a script

Every tool above is also reachable as data, through the `MeshEditControl`
resource. Write to it and a system translates: a mode name becomes a viewport
mode, an op name goes into the same queue the buttons push into, a list of
points becomes a stroke. Nothing there is a second implementation — an op run
this way lands on the undo stack and updates the selection exactly as a click
would.

It exists because the interactive tools are all gesture-driven, and a script,
a test, or an agent driving the editor over MCP cannot press Tab. It also makes
a sculpt *reproducible*: the same list of points gives the same result every
time, where a drag never does.

| Field | Meaning |
|---|---|
| `mode` | `scene`, `edit` or `sculpt` |
| `select_mode` | `vertex`, `edge` or `face` |
| `select_all` | Select every element of the current mode |
| `select_facing` / `facing_tolerance` / `facing_outermost` | Select the faces pointing a given way; `facing_outermost` keeps only the furthest along it, so stacked insets do not re-select the rings they left behind |
| `ops` | `subdivide`, `inset`, `extrude`, `delete`, `dissolve`, `merge`, `remove_doubles`, `array`, `bisect_x/y/z`, `mirror_x/y/z` |
| `amount` | How far `extrude` moves |
| `sculpt_at` | Dab points, in the mesh's **local** space |
| `brush` | `draw`, `clay`, `crease`, `scrape`, `smooth`, `mask`, `grab`, `snake_hook`, `inflate`, `flatten`, `pinch` |
| `grab_delta` | Where `grab` and `snake_hook` drag to |
| `brush_radius` / `brush_strength` / `brush_invert` | As the Inspector sliders |
| `detail` | Dyntopo detail size for these dabs, as an absolute edge length |
| `remesh` | Redistribute the whole surface into triangles of about this edge length |
| `symmetry_x` | Mirror every dab across the local X plane (the Inspector's X Symmetry toggle does the same) |
| `mask_op` | A whole-mesh mask change to run before the dabs: `clear`, `all`, `invert`, `smooth`, `sphere` |
| `mask_at` / `mask_radius` / `mask_value` | The `sphere` op's centre, radius and value |
| `wireframe` / `solid` | `on` or `off` for the viewport overlays |
| `visualization` | Viewport visualization mode by name; `matcap` is the one to use while sculpting (see [Visualization modes](viewport.md#visualization-modes)) |
| `last_result` | What the last request did, or why it did nothing |

**Read `last_result` back.** An op that found nothing selected is otherwise
silent, and this is the only channel that says so.

Dab points are snapped onto the nearest surface within twice the brush radius
before the brush runs. The interactive path never needs this — it raycasts the
pointer, so its dab is on the mesh by construction — but a script is naming
coordinates blind, and the surface has usually moved since it last looked,
because every earlier dab in the same stroke deformed it. Taken literally, a
stroke that starts correctly walks off the surface a fraction at a time and the
rest of it silently does nothing. The snap distance is reported in
`last_result` when it is not negligible; a large one means the caller's idea of
where the surface is has drifted a long way from where it actually is. The
limit is what stops a snap from crossing a gap and carving the torso when you
aimed at an arm.

To pull a limb, walk `snake_hook` along a direction, one write per step, with
`detail` set to about a quarter of `brush_radius`.

To isolate a region before working on it, set `mask_op` to `all` and then run a
`sphere` with `mask_value` 0 over the part you want free. Turn `symmetry_x` on
and you write one side; without it a symmetric model means issuing every dab
twice with the sign flipped by hand, and the two sides drift apart the moment
one list is edited and the other is not.

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
