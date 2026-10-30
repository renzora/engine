# Virtual Geometry

Virtual geometry lets a model carry far more triangles than ordinary rendering
could afford. The mesh is cut into clusters of up to 128 triangles, and each
cluster is drawn at the level of detail its distance from the camera calls
for: full detail up close, simplified further away, never more than about a
pixel of difference from the full-detail surface. Clusters outside the view
are skipped. All of this happens on the GPU, every frame, for every camera.

It is on by default, for every model in a project. Nothing has to be set up.

## Models are stored as `.rvg`

Importing a model in any format (FBX, OBJ, glTF, USD, ...) stores it as one
`.rvg` file:

```
models/bistro/Bistro.rvg
models/bistro/materials/...
models/bistro/textures/...
```

An `.rvg` model holds two things: the model's scene (its nodes, names,
materials, skeleton, blend shapes and cameras, as glTF) and its geometry as
clusters with their levels of detail. The asset browser shows it as a
**Model** with a thumbnail, and it is dragged into a scene, placed and
animated exactly as a `.glb` was. Building takes a few seconds for a large
model (the 2.8 million triangle Bistro scene takes about five), once, at
import.

Each mesh in the scene keeps a coarser copy of itself (about a third of its
triangles for Bistro; small meshes keep all of them), which is what shadows,
physics and picking read while the clusters draw the detail. Solari traces the
full detail instead, rebuilt from the clusters while it is switched on: the
coarser surface sits slightly off the drawn one, and traced it would cast
black patches across the model. The full-detail
geometry is stored once, in the clusters. For Bistro's exterior the `.rvg` is
87 MB, where the `.glb` plus its virtual geometry used to be 211 MB. Textures
are separate `.rmip` files beside it, as before.

The rigging, weight painting and blend shape tools edit an `.rvg` model in
place, as they edited a `.glb`: they work on the full-detail model and store
the result back as `.rvg`. **Export with Clips** still writes a `.glb`, for
use outside the engine.

### Projects with `.glb` models

A project from before models were stored as `.rvg` keeps working as it was:
each `.glb` has an `.rvg` beside it holding only its virtual geometry, which
the editor builds by itself, in the background:

- when a project opens, every `.glb` is checked, and any without a current
  `.rvg` is converted;
- a `.glb` added or edited while the project is open is converted again when it
  changes;
- an export checks every `.glb` before it packs the project.

A `.glb` model renders as ordinary geometry until its `.rvg` is ready, then
switches over without a reload. That `.rvg` is current when it was built from
exactly this `.glb` by this version of the builder, both recorded in its
header, and deleting it is always safe: the editor rebuilds it. The asset
browser hides it, since it is part of the `.glb` beside it, and it gets no
thumbnail. It holds no scene, so it is not a model to place: place the `.glb`.
A scene entity that names it anyway loads nothing, with a warning in the log.

The same goes for a `.glb` copied into a project folder without importing it,
from a file manager or a tool. It gets an `.rvg` beside it like any other, and
renders with the materials inside it: only an imported model has the
`materials/` folder of editable `.material` files its meshes are bound to.

When such a project opens, the Console says how many models are stored as
`.glb`. **Convert Models to RVG...** in the command palette (`Ctrl+P`) turns
them into `.rvg` models, after asking:

- each `.glb` is built into an `.rvg` of the same name, replacing its sidecar;
- every scene, material and script that names the model is updated to the new
  path, the open scene included, exactly as when a file is renamed in the
  asset browser;
- only then is the `.glb` deleted. A model that fails to convert is left as it
  was, with the reason in the Console.

`.glb` files under `exports/` are left alone. If an `.rvg` model of the same
name already exists, both files are kept and the Console says so.

## Seeing it work

The viewport's **Visualization** menu has five virtual geometry views. Each
colours what virtual geometry drew and greys out every other mesh, so ordinary
geometry (and anything that did not convert) stands out. The sky keeps its
colour:

| View | Colours by | What to look for |
|---|---|---|
| **Virtual Geometry: Clusters** | cluster | Patches of up to 128 triangles. Up close they are small; far away a single cluster covers a large area, because a coarser level was chosen. |
| **Virtual Geometry: Triangles** | triangle | The actual triangle density on screen. It stays roughly even as you move, which is the point. |
| **Virtual Geometry: LOD Level** | level of detail | Green is full detail, then yellow, orange, red and purple for each coarser level. Pull the camera back and watch the colours move outward. |
| **Virtual Geometry: Instances** | converted mesh | Which meshes are virtual geometry at all. |
| **Virtual Geometry: Residency** | streaming | Green where the detail the camera wants is in GPU memory, red where a coarser level stands in for it. Red flashes and turns green as you move; red that stays means the memory budget has no room for that detail (see Streaming). |

The views apply to editor viewports only; the Game panel and exported games
never show them.

## Streaming

Only the detail the cameras need is kept in GPU memory, within a budget set
per project:

```toml
[rendering]
virtual_geometry_budget_mb = 512   # the default
```

A model arrives with only its coarsest level loaded, which is always kept.
Each frame the renderer notes which parts of the screen want more detail than
is loaded, and the finer data is uploaded over the next frames, coarse levels
first, at most 16 MB a frame so a sudden camera cut does not stall. A region
asked for brings the finer levels below it along in the same frame, while the
16 MB and free memory allow, so a model reaches full detail in a frame or two
rather than a level every few frames. Until it arrives the coarser level stays
on screen in its place: detail sharpens in, but nothing is ever missing. When
the budget is full, the detail drawn least recently is evicted to make room;
detail brought along but never drawn ages out the same way.

With Solari on this matters beyond sharpness: Solari traces the full detail,
so while a coarser level is drawn the two do not line up, and light leaks
through the gap as bright or black patches. For the same reason, while Solari
is on, virtual geometry draws its finest level wherever it is loaded, rather
than the coarsest one within a pixel of it: even a fraction of a pixel between
the drawn and the traced surface showed as black patches and gaps in curtain
folds and at grazing edges. Clusters still stream and cull per view, so this
costs triangles drawn, not memory.

The budget caps memory; it is not a target. A scene that fits uses what its
views need and no more, so most projects never reach it. Lower it for
low-memory hardware: the cost is detail that stays coarse up close, never a
hole.

## Terrain

Terrain chunks are drawn as virtual geometry too, from clusters the editor
builds as the ground settles and saves under `terrain_clusters/`. See
[Terrain: Virtual geometry](../editor/terrain.md#virtual-geometry).

## Mixing with ordinary geometry

Virtual and ordinary geometry render side by side in one scene, and share
lighting, shadows, post-processing, TAA, DLSS and Solari. They occlude each
other correctly, because both write the same depth buffer.

Some things stay ordinary geometry for now, automatically:

| Stays ordinary | Why |
|---|---|
| Node-graph materials that read particle inputs | Particle inputs come from the particle pipeline's own vertex stage. |
| Plugin materials | A material type converts only once it is registered for virtual geometry (below). |
| Wind-animated meshes | Their vertices move every frame. |
| Transmissive materials (refraction: **Specular** or **Diffuse Transmission** above zero) | They refract a copy of the scene behind them, which Bevy draws in a pass of its own. Plain blended glass converts (below). |
| **Alpha Mode: Alpha To Coverage** | It needs multisampling, which views drawing virtual geometry do not use. |
| Meshes with morph targets (blend shapes, such as a face rig's) | Their vertices move by amounts the clusters have no record of. The rest of the character still converts. |
| Points and lines | Not triangles. |

A model is converted mesh by mesh: if half of it qualifies, that half is
virtual geometry and the rest renders as before. The Console says which meshes
of a converted model stayed ordinary geometry and why, one line per model.

### Materials

Plain materials and node-graph materials both convert, opaque or cut-out. A
node graph runs exactly as it does on ordinary geometry: its generated shader
gains a second entry that reads the surface from virtual geometry, and the
graph body is unchanged. Graph nodes that sample textures use the screen's own
derivatives there, so a pixel on the very edge of a surface may pick a slightly
blurrier mip level than it would on ordinary geometry.

**For plugin authors:** a material type opts in with

```rust
use renzora_virtual_geometry::{VirtualGeometryAppExt, VgSurface};

app.add_virtual_geometry_material::<MyMaterial>(|material| {
    // None keeps this material's meshes ordinary geometry.
    Some(VgSurface { masked: false, double_sided: false, transparent: false })
});
```

Its shaders must handle `MESHLET_MESH_MATERIAL_PASS` the way Bevy's PBR
shaders do (taking `VertexOutput` from
`bevy_pbr::meshlet::visibility_buffer_resolve`), and its bind group must start
with `StandardMaterial`'s bindings, which the cut-out pass and the prepass
read. A `transparent` material is shaded by its ordinary forward fragment
shader instead, so it needs neither.

### Foliage and other cut-outs

Alpha-tested materials (**Alpha Mode: Mask**: leaves, grass, fences, chain
link) convert too. They are rasterized by a pass that reads the material's
base colour alpha and drops what falls below the cutoff, so each card covers
only the shape cut into its texture.

They are simplified differently from solid meshes. The outline of every card is
kept exactly, so a tree does not thin out or shrink as it recedes; only the
inside of larger connected leaf meshes is simplified. The trade is that a
canopy of separate cards reaches its coarsest level early and stays at about
its full triangle count far away. It still gains the culling.

### Glass and other blended materials

Materials with **Alpha Mode: Blend**, **Premultiplied**, **Add** or **Multiply**
(windows, bottles, visors, water surfaces, holograms) convert too. They get
the same culling and level of detail, and are then drawn in the ordinary
transparent pass, sorted back to front with every other transparent object
in the scene, virtual or not, exactly as ordinary geometry is: one sort per
mesh, by the centre of its bounds.

The virtual geometry debug views do not colour them, since they never reach
the buffer those views read; they show greyed out like ordinary geometry.

### Animated characters

Skinned meshes (anything bound to a skeleton: characters, creatures, a
swinging sign) convert like any other mesh, and animate exactly as before.
Every frame the GPU moves each of their vertices by its joints, the same
calculation ordinary rendering does, and the clusters are drawn from the moved
vertices. Motion vectors come from the previous frame's pose, so TAA, DLSS and
motion blur see the animation.

Two differences from static meshes:

- The level of detail is picked from the character's resting pose, through its
  transform. It is right for anything that moves about its own origin, which is
  almost everything.
- Clusters are not culled one by one against the view, because a joint can
  carry a cluster far from where it rests (an arm raised over the head). The
  character as a whole is still culled as an ordinary skinned mesh is.

When a skinned mesh is built, the simplifier is told to keep evenly sized
triangles, so coarser levels still bend cleanly at the joints.

While any mesh is virtual geometry, 3D cameras render without MSAA (the same as
in deferred mode). Every editor viewport and game camera already does; only a
few preview cameras used it.

## Turning it off

- **For one model in a scene:** add the **Standard Geometry** component to the
  model, or to any entity above it. Everything under it renders as ordinary
  geometry.
- **For the whole project:** in `project.toml`,

  ```toml
  [rendering]
  virtual_geometry = false
  ```

  `.rvg` models still load, and draw at full detail as ordinary geometry: the
  full-detail mesh is rebuilt from the clusters when the model loads. Turning
  it back on switches them back without a reload. `.glb` models are not
  converted while it is off, and their sidecar `.rvg` files go unused.

The same full-detail fallback is used wherever virtual geometry does not draw
a mesh: under **Standard Geometry**, for a mesh whose material stays ordinary
geometry, and on the web.

## Exporting

An export packs each `.rvg` model as it is, and each `.glb` model with its
sidecar `.rvg`. The lean export's
**Virtual Geometry** feature (under 3D rendering) is detected automatically
when the project has models; untick it to leave the renderer out of the game,
which then draws every model at full detail as ordinary geometry.

Virtual geometry is not available on the web export: there, every model renders
as ordinary geometry, `.rvg` models at full detail.

## Current limits

- **Shadows** are drawn from each mesh's coarser copy (see above). They are
  correct, but do not yet benefit from the level of detail, and a shadow's
  edge can differ slightly from the full-detail surface.
- **Solari** traces an ordinary full-detail mesh, built while Solari is on,
  so an `.rvg` model's geometry is in memory a second time for as long as it
  is. It does not use the level of detail. The drawn surface can be up to a
  pixel off the traced one, so Solari starts its rays two pixels' width off
  every surface; raising the renderer's `lod_error_pixels` above 2 would bring
  back black patches on distant geometry.
- The coarser copy is stored beside the clusters, so an `.rvg` model is
  larger than its clusters alone. It is compressed, and reads in under a
  second for Bistro.
- The whole `.rvg` is held in system memory while its model is loaded;
  streaming decides only what is on the GPU. Reading pages from disk on demand
  comes later.
- A page of streamed detail stands alone, so vertices shared between pages are
  stored once in each. With everything loaded, Bistro's streamed data is 188 MB
  where one shared copy was 125 MB; in practice only what the views need is
  loaded. Its coarsest level, always resident, is 32 MB.
- Skinned meshes are not streamed: each stays whole in GPU memory.

## How it works

For contributors. The code is in `crates/renzora_rvg` (the file format and the
builder), `crates/renzora_virtual_geometry` (the renderer) and
`crates/renzora_virtual_geometry/editor` (the background converter for `.glb`
models, and the conversion to `.rvg` models).

**The model file.** An `.rvg` model's compressed body ends with its scene, a
GLB in which each converted primitive's triangles are its proxy (a coarse cut
of the cluster DAG, compacted to the vertices it uses). `renzora_rvg::model`'s
loader hands that GLB to Bevy's own `GltfLoader::load_gltf` with the `.rvg`'s
load context, so the root asset of `x.rvg` is a `Gltf` with Bevy's usual
labels (`x.rvg#Scene0`, `x.rvg#Mesh0/Primitive0`), and the clusters are the
labeled sub-asset `x.rvg#Clusters`. Everything that loads a `Handle<Gltf>`
works on it unchanged. A sidecar `.rvg` (no scene) loads the same way, as an
empty scene plus its clusters. `renzora_rvg::bake::full_detail_glb` rebuilds
the original GLB from the scene and the full-detail clusters, which the rig
tools edit.

**Building.** meshoptimizer's cluster LOD builder clusterizes each glTF
primitive, groups neighbouring clusters, simplifies each group with its outer
border locked, and repeats on the result until nothing simplifies further. That
gives a DAG of clusters in which every cluster records its own error and its
group's simplified error, each with a bounding sphere. Because errors only grow
up the DAG and every cluster of a group shares its group's bound, picking each
cluster on its own (draw it when its own error is under the threshold and its
group's is not) always yields a crack-free surface.

**Per frame, per camera:**

0. If anything is skinned, a compute pass deforms every skinned instance's
   vertices with Bevy's own joint matrices (current and previous frame) into a
   shared buffer, in world space. The passes below read a skinned instance's
   vertices from there instead of transforming its rest pose.
1. A compute pass walks every (instance, cluster) pair, projects both errors to
   pixels, keeps the clusters that are part of the cut and inside the frustum,
   and appends them to a list feeding two indirect draws (single-sided and
   double-sided), plus one per bucket of alpha-masked clusters (a bucket being
   one material bind group and cull mode; with bindless materials, many
   materials share one).
2. A raster pass draws those clusters into a visibility buffer (which cluster
   and triangle covers each pixel) and into the view's own depth buffer. It
   runs first in the prepass, so ordinary meshes then depth-test against it.
3. A pass writes each covered pixel's material id as depth.
4. Each material's own Bevy shader runs once over a fullscreen triangle at its
   material depth, so the depth test leaves it only its own pixels. Compiled
   with `MESHLET_MESH_MATERIAL_PASS`, Bevy's PBR shaders read their surface
   (position, normal, UV and its screen-space derivatives, tangent) from a
   resolve module this crate supplies instead of a vertex stage, and write the
   same targets an ordinary mesh would: lit colour in forward, the G-buffer in
   deferred, normals and motion vectors in the prepass.

Transparent clusters skip steps 2 to 4: the cull pass puts each transparent
instance's clusters in a bucket of its own, and the instance is added to Bevy's
transparent phase as an item, drawn by its material's forward pipeline with a
vertex stage that reads the bucket's clusters.

**Streaming.** Every cluster also records the group whose simplification made
it. Groups are loaded and evicted whole, each as a self-contained page in a
pool, and the cut rule gains one case: a cluster whose own error is too large
is still drawn while the finer clusters that would replace it are not
resident, as their stand-in. The cull pass asks for those, and marks what it
drew; both come back to the CPU a frame or two later through a mapped buffer,
and the streaming manager loads and evicts accordingly. The rule leaves no
hole as long as residency is closed upward (a group is resident only while the
groups its simplification fed are), which the manager keeps by loading parents
before children and evicting children before parents. `renzora_rvg`'s
`GroupGraph` holds the rule, and a test checks it on random resident sets:
every cut is watertight, and breaking the closure opens a crack.

A converted entity keeps its `Mesh3d`; only its visibility class changes, which
takes it out of Bevy's camera passes while physics, picking and shadows keep
using the mesh. For an `.rvg` model that mesh is the proxy; when virtual
geometry does not draw it, `full_detail` swaps in a full-detail mesh built
from the clusters (cached per mesh), and swaps the proxy back when it does.
A converted proxy also carries the contract component
`renzora::TracedGeometry`: empty, it tells a ray tracer not to trace the
proxy, and while `renzora::TracedGeometryWanted` exists (Solari inserts it
when switched on) `traced_geometry` fills it with the full-detail mesh, which
Solari traces in place of `Mesh3d`.
