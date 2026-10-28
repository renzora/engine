# Solari — hardware ray-traced global illumination

Solari is Renzora's global illumination: the engine's only GI, and it is
**hardware ray-traced**. It wraps Bevy's experimental `bevy_solari`
(`SolariPlugins`) and is **built into the engine** as the `renzora_solari`
crate, with its inspector in `renzora_solari_editor`. Solari traces against a
real BVH of the scene: fully dynamic direct **and** indirect lighting from
emissive meshes and directional lights, with no baking.

> **Experimental.** Solari requires a recent ray-tracing-capable GPU and is still
> evolving upstream. It used to be a marketplace plugin; it now ships with every
> build, so there is nothing to install, and an older installed copy under
> `plugins/solari` should be deleted.

## How to enable

1. **Build the engine.** `renzora_solari` is an in-workspace plugin, compiled
   into the binary by any ordinary `cargo renzora dist`. It sits behind
   `renzora_runtime`'s `solari` feature, so the lean exporter can leave it out
   of a game that never uses it: an export keeps Solari only when one of its
   scenes carries the *Solari Ray-Traced GI* component.
2. **Run on an RT-capable GPU.** At startup the host probes the GPU adapter
   (`raytracing_supported()`); on a GPU that reports the ray-tracing wgpu features
   it requests them and sets `renzora::GpuRaytracing { enabled: true }`. If the
   GPU can't do ray tracing, the plugin logs a warning and stays **inert** — the
   engine still boots normally.
3. **Author `Solari Ray-Traced GI`.** Select the **World Environment** entity and
   add the *Solari Ray-Traced GI* component (Inspector → Add Component →
   Lighting). Toggle it on.

## Why a GPU capability flag

A plugin's `build()` runs **after** the render device's feature set has been
decided. Ray-tracing wgpu features
(`EXPERIMENTAL_RAY_QUERY` + acceleration structures) can only be requested *at
device-creation time*, and Bevy ORs the requested feature set into
`required_features` without intersecting against adapter support — so requesting
them on a GPU that lacks them would **fail device creation** and crash the engine.

The host therefore makes the capability decision once, before plugins load:

- `renzora_runtime::raytracing_supported()` spins up a throwaway wgpu adapter on
  the selected backend and checks `SolariPlugins::required_wgpu_features()`.
- When supported, `platform_wgpu_settings()` adds those features to `WgpuSettings`
  and the runtime inserts `renzora::GpuRaytracing { enabled: true }`.
- `renzora_solari` reads that resource in `build()` and installs `SolariPlugins`
  **only** when ray tracing is live. Otherwise it warns and no-ops.

The plugin owns the *GI behaviour*; the host owns the one-time GPU capability
decision, which is the half no plugin can make.

## What the plugin does when active

- **Settings.** `SolariGi` has four fields: `enabled`; `quality` (**Quality**,
  `High` by default, see [Quality](#quality)); `suppress_shadow_maps`
  (**Suppress Shadow Maps**, on by default, see [Performance](#performance));
  and `light_proxies` (**Point/Spot Light Proxies**, on by default, see
  [Light proxies](#light-proxies)).
- **Camera.** `SolariGi` is routed from the World Environment onto each active
  camera via `EffectRouting` (the same path every camera effect uses). On a routed camera the
  plugin inserts Bevy's `SolariLighting` — whose `#[require]`s pull in HDR and the
  deferred/depth/motion-vector prepasses — and forces **`Msaa::Off`** (Solari
  mandates no MSAA). Disabling it removes `SolariLighting` and restores the
  default MSAA.
- **Meshes.** While Solari is active, eligible meshes are mirrored into the
  ray-tracing scene with `RaytracingMesh3d` (it coexists with the rasterized
  `Mesh3d`). Solari's BLAS builder demands `TriangleList` topology, 32-bit
  indices, and **exactly** the attribute set `{POSITION, NORMAL, UV_0, TANGENT}`.

  That last requirement is stricter than it looks, and it fails silently. Bevy
  compares the mesh's *whole* attribute list in id order, so anything extra
  disqualifies it: a second UV set (id 3, which sits between `UV_0` and
  `TANGENT` — and which [wind](wind.md) reads), vertex colours, or skinning
  weights. No BLAS is built, nothing is logged, and every instance of that mesh
  is quietly dropped from the acceleration structure.

  Renzora repairs this in two ways:

  - **In place**, when the mesh only lacks tangents or uses 16-bit indices —
    tangents are generated and indices promoted on the asset itself. This is
    what most imported GLBs need and it costs no extra memory.
  - **Via a stripped copy**, when *extra* attributes are the problem. Those
    can't be removed from the shared asset without breaking the rasterized
    draw, so a ray-tracing-only duplicate is built with just the four
    attributes the BLAS wants, and `RaytracingMesh3d` points at that instead.
    The copy is cached per source asset, shared by every instance, and freed
    when Solari goes idle.

  A mesh with no normals or no UVs gets them made up in that copy (smooth
  normals, UVs of zero), and tangents come from the normals where the UVs can't
  give any. Only geometry that is not an indexed triangle list is marked
  skipped, so it isn't re-checked every frame, rather than crashing the builder.
- **What counts as "eligible".** Bevy's ray-tracing scene is global and
  unfiltered: every entity carrying `RaytracingMesh3d` goes into the TLAS at full
  ray mask, with no regard for `Visibility` or `RenderLayers`. Renzora therefore
  gates the mirror itself. A mesh participates only when all of the following
  hold:

  | Requirement | Why |
  |---|---|
  | `MeshMaterial3d<StandardMaterial>`, or a `TracedStandIn` | The only material type Solari's binder reads. A node-graph material (`GraphMaterial`) carries a `TracedStandIn` while Solari is on, holding a plain `StandardMaterial` copy of the graph's base approximation; the mesh is then traced through a separate traced-only twin entity with that stand-in, so it is not drawn twice. Reflections and bounce light see the approximation; what the camera sees directly is the graph itself, through the G-buffer. |
  | Visible (`InheritedVisibility`) | An entity hidden with the hierarchy eye, or a drag-and-drop model ghost, must not cast ray-traced shadows. Frustum culling (`ViewVisibility`) is deliberately **not** consulted — off-screen geometry has to stay in the BVH, since shadowing and bounce from outside the view is the point of ray tracing. |
  | On render layer 0 | Layer 0 is the scene; layers 1+ are gizmos and the editor's offscreen rigs (asset-browser model thumbnails, the material viewer, the animation studio preview). Those live in the same `World`, at or beside the world origin, and would otherwise become invisible solid occluders standing in the middle of your level. |

  Every alpha mode participates. `Opaque`, `Mask` and `AlphaToCoverage` are
  traced as solid. A blended one (`Blend`, `Premultiplied`, `Add`, `Multiply`)
  is traced as [see-through](#glass-and-other-see-through-surfaces): shadow and
  bounce rays pass it by, and Solari lights it itself. A blended material that
  is **emissive** is traced as solid instead, because an emissive mesh is an
  area light, and neon tubes and lamp glass must stay where rays reach them.

  All three are re-checked every frame, so unhiding an object or changing its
  alpha mode updates the ray-traced scene immediately.
- **Virtual geometry.** A model stored as `.rvg` draws its detail from
  clusters, and its `Mesh3d` is only a coarser copy. Traced, that copy would
  sit slightly off the drawn surface and shade it with black, polygon-shaped
  patches, so such a mesh carries `renzora::TracedGeometry` and is traced as
  the full-detail mesh it holds, which virtual geometry builds while Solari is
  on (Solari inserts `renzora::TracedGeometryWanted`). Until it is built, the
  mesh is left out rather than traced coarse. A mesh whose `Mesh3d` changes is
  traced again from the new one.
- **Idle.** When no camera has Solari enabled, the ray-tracing mirror is dropped
  so BLAS resources are freed.

## Glass and other see-through surfaces

A mesh with a blended material (window glass, a car's clear-coat shell, a shop
front) reflects the scene around it while Solari is on: the road under a car,
the buildings beside it, a neon sign across the street. Without Solari, and
before this, it could only reflect the sky.

How it works: Solari's G-buffer holds only the nearest opaque surface, so a
blended one cannot be lit there. Instead each blended mesh goes into the traced
scene under its own ray mask, which every shadow and bounce ray ignores, so
glass does not cast a solid black shadow. After the opaque image is lit and
denoised, one more pass traces each pixel's camera ray for the see-through
surfaces in front of the opaque one, nearest first and up to four of them
(a headlight is a clear lens over a tinted one, each with a front and a back
face), and lights each:

- its **reflection**: a ray along the mirror direction (four spread over the
  highlight for a surface rougher than about 0.15), which takes the light of
  what it hits from Solari's world cache, or the sky where it leaves the scene;
- **highlights** of the sun and other lights;
- its own **tint**, lit by the light arriving at it. A surface with a real
  colour of its own, such as a sheer curtain or frosted glass, gathers that
  light with eight rays over the side facing the camera (the sky included),
  the sun and any other directional light, and four samples of the scene's
  other lights (lamps and emissive surfaces), and half as much again from the
  side facing away, the light through the weave that makes a curtain in a
  window glow. A gather ray that meets a lamp or light panel does not add its
  glow, since the light samples already count it. They are drawn the same way every
  frame for each point on the surface, so the result is steady rather than
  grainy-and-flickering; clear and metallic glass skip this and cost nothing
  extra;
- a **clear coat**, when the material has one: a mirror-smooth lacquer over
  all of the above, reflecting about 4% head on and more at grazing angles,
  and dimming what is under it by as much. A car window is usually rough, dark
  glass under a full clear coat, and the coat is what gives it its crisp
  reflection. The coat's own roughness is not read; it is taken as smooth.

A ray meets every face, but a face the material culls when Bevy rasterizes it
(the back of a single-sided pane) is skipped here too, so a model looks the
same with Solari on as off. A double-sided material draws both faces in both:
a lens modelled as a closed, double-sided slab is crossed twice, and each
crossing tints what is behind it.

Each is blended over what lies behind it the way its alpha mode says, the same as
Bevy's forward pass would. As there, a blended material's alpha weighs its
whole shading, its reflection included, so a glass with a low alpha reflects
faintly. A dark tint over a darker interior is what makes a window's
reflection read strongly.

Where nothing opaque is behind them, as with a window or a curtain against the
sky, see-through surfaces are drawn in a second pass once the sky has been
drawn; the sky would otherwise paint over them.
While Solari draws a mesh this way it carries `renzora::SolariShadedTransparent`,
which takes it out of Bevy's raster passes (and virtual geometry's), so it is
not drawn twice. A mesh still waiting for its ray-tracing copy is drawn the old
way meanwhile.

Limits:

- At most **four** see-through surfaces along each pixel are drawn; a fifth
  and beyond are not. A layer is skipped once almost nothing behind it could
  show, so dense stacks of dark glass cost no more than they need.
- The reflection is not denoised. A sharp one is clean; a rough one is a little
  grainy.
- Reflected surfaces are lit from the world cache, which is coarser than the
  directly lit image, so a reflected shadow edge is soft. The pass only reads
  the cache: it never adds cells or keeps them alive, so glass cannot change the
  rest of the image's lighting. Where the cache has nothing yet, a reflected
  surface is lit by one sample of the scene's lights, without its bounce light.
- No refraction: what is behind the glass is seen straight through.

## Quality

**Quality** on the Solari component picks how much work Solari does per frame.
It trades frame rate against noise, and you can change it while the scene is
running.

| Preset | ReSTIR | Bounces | Light samples (primary / bounce) | Use it for |
|---|---|---|---|---|
| Low | Off | 1 | 2 / 1 | Moving around a scene. Fastest, and visibly grainy. |
| Medium | On | 2 | 4 / 2 | Everyday editing. |
| High (default) | On | 3 | 8 / 4 | Judging lighting. These are Bevy's own defaults. |
| Ultra | On | 4 | 16 / 8 | Stills and close-ups. |

The world cache that stores bounce light scales with the preset too. It holds
about a million cells, and once it is 40% full, bounce rays stop adding the
cells they land on, which slows how fast a large enclosed scene such as a
city street fills it. A lookup searches up to eight slots for its cell, so
even a well-filled cache still makes the cells newly seen surfaces need;
where it cannot, they show as dark patches crawling over them while the
camera moves. The panel's **World cache cells** figure shows how full it is,
and its **World cache** view shows any missing cells as black.

**ReSTIR is the setting that matters most.** It reuses light samples across
neighbouring pixels and earlier frames, so the [denoiser](#denoiser) starts
from far less noise. `Low` turns it off, which is why `Low` is the grainiest
preset and not just the cheapest one.

The **ReSTIR** toggle below Quality overrides the preset either way: ReSTIR on
with `Low`'s other savings, or off on `Ultra` to see what it contributes.
Picking a Quality preset puts the toggle back to that preset's own setting.

**Glass Reflections** (on by default) ray traces see-through materials, those
with alpha mode Blend: glass, clear coats, lenses. They then reflect the scene
around them and show what is behind them. Solari's own image holds only opaque
surfaces, so with this off they are drawn by the ordinary forward pass, which
reflects only the sky. Opaque surfaces reflect the scene either way.

Solari's cost also scales with the number of pixels it lights, so lowering the
camera's **Resolution** is often a bigger saving than dropping a preset.

## Lights Solari can see

This is the single most surprising thing about Solari, and the usual reason a
scene comes out darker than the raster pipeline. `bevy_solari` 0.20 samples
**three** kinds of light source:

| Source | Supported |
|---|---|
| `DirectionalLight` (the sun) | Yes |
| Emissive meshes | Yes, sampled as real area lights |
| Sky (the environment map) | Yes, at the Environment Map's intensity. See [Sky light](#sky-light). |
| `PointLight` | Not natively. Renzora stands one up as an emissive sphere (below). |
| `SpotLight` | Not natively. Same, but the cone is lost. |
| `AmbientLight` | **No**, and there is no workaround |

### Sky light

A ray that leaves the scene samples the camera's environment map, so the sky
lights everything the sun doesn't reach. In Renzora that is the **Environment
Map** section of the World Environment, which bakes the atmosphere into a
cubemap.

Two settings decide how much sky Solari gets:

- **Environment Map switched off** means no sky light at all. Everything not
  facing the sun is then lit by bounce alone, and shadowed facades go close to
  black while the sunlit ground looks right. The plugin warns about this in the
  Console, except while the sun is below the horizon: a night sky gives no
  light, so a dark sky at night is expected and not a missing setting.
- **Intensity** scales the sky directly. `1.0` is the physically correct sky.
  Lower values that looked fine under raster lighting make a Solari scene
  noticeably darker in shadow.

Point and spot lights aren't dimmed or approximated by Solari — its binder never
looks at them — and because a Solari camera carries `SkipDeferredLighting`,
Bevy's clustered-light pass (the only other thing that would apply them) doesn't
run either. Left alone, a street lined with lamp posts renders unlit, which is
easy to mistake for broken GI. It also starves [bloom](#bloom): with the lamps
contributing nothing, very little of the image gets bright enough to cross the
bloom threshold, so bloom looks broken too.

### Light proxies

**Point/Spot Light Proxies** (on the `SolariGi` component, on by default) fixes
this. Each point or spot light gets a small sphere in the ray-tracing scene whose
emissive radiance is derived from the light's luminous power, and Solari samples
that as a first-class area light — soft shadows and colour bleed included.

The sphere carries `RaytracingMesh3d` **without** `Mesh3d`, so it is real to the
ray tracer and does not exist as far as the rasterizer is concerned: no glowing
ball appears in the viewport. Proxies are spawned as root entities rather than as
children of your lights, so your hierarchy is untouched, and they carry no
`Name`, so scene save ignores them.

The conversion is photometric, not a fudge factor. Bevy stores light intensity as
luminous power in lumens and converts to luminous intensity with `I = P / 4π`; a
uniformly-emitting sphere of radius `r` and radiance `L` has `I = L·πr²` in every
direction, so the proxy uses:

```text
L = P / (4 π² r²)
```

The sphere takes the light's `radius`, clamped to a 2 cm minimum — radiance goes
as `1/r²`, and Bevy's default `radius` is `0.0`.

Two things it does not solve:

- **A spot light becomes omnidirectional.** Bevy applies the cone as an angular
  mask over the same `P / 4π` intensity, so the in-cone brightness is right, but
  a sphere can't carry the mask and the light spills outside the cone. For a
  downlight that means some illumination where there shouldn't be any. Author
  real emissive geometry if that matters, or turn the toggle off.
- **`AmbientLight` cannot be represented this way at all.** The only shape it
  could take in a traced scene is an enclosing emissive dome, which would then
  sit between every surface and the sun. Use the environment map for sky and
  fill light instead (see [Sky light](#sky-light)); the plugin warns once if it
  finds an `AmbientLight` on a Solari camera.

  Watch out for auto-exposure making a dim Solari scene worse rather than better.
  `AutoExposureSettings.keep_dark_strength` deliberately *stops* metering from
  lifting a dark scene — at the defaults it applies up to several stops of extra
  negative compensation once the metered scene falls below `keep_dark_pivot_ev`.
  That is the right behaviour for night, but under Solari it compounds a scene
  that is dim for an unrelated reason. Lower `keep_dark_strength` when comparing
  the two backends.

You can always author light by hand instead: an emissive `StandardMaterial` on
the bulb or fixture geometry is sampled exactly the same way, and gives the light
a real shape.

## Bloom

Bloom is not special-cased for Solari and nothing disables it — but it is
threshold-driven, and Solari changes how much of the image clears the threshold.
If bloom seems to stop working when you enable Solari, the cause is almost always
that the scene got darker rather than that bloom broke:

- Point and spot lights contribute nothing unless
  [light proxies](#light-proxies) are on. Those lamps are usually what was
  blooming.
- With the Environment Map off or turned down, everything in shadow sits lower
  than it did under the raster pipeline (see [Sky light](#sky-light)).
- Emissive surfaces still bloom normally — Solari adds `material.emissive` and
  applies view exposure exactly as the raster path does.

If you want the same look at a lower light level, drop `BloomSettings.threshold`
rather than raising light intensities.

## Performance

Almost all of Solari's cost is on the GPU, in its compute passes. The scene
upload is incremental: only changed transforms, materials and lights are
uploaded, and the TLAS instances are set up on the GPU. As a reference, the lit
Bistro night scene (90 point lights) on an RTX 3060 at about 1300x930 runs at
roughly 40 ms a frame on `High`, against 34 ms with Solari off.

Things worth knowing:

- **Shadow maps are suppressed while Solari is on** (`Suppress Shadow Maps` in
  the inspector, on by default). A Solari camera carries `SkipDeferredLighting`,
  so the standard lighting pass, the only consumer of a shadow map, never
  runs. Without suppression every directional cascade and every point-light
  cubemap is rendered in full and then discarded, which in a scene with a lot of
  lamps is a serious amount of wasted GPU time (in the Bistro scene above it
  more than tripled the frame time). Bevy's own guidance is the same: set
  `shadow_maps_enabled: false` on all lights while Solari is active.

  The suppression is applied in the **render world**, to the extracted lights.
  Your `PointLight` / `DirectionalLight` components are never written, so a scene
  saved while Solari happens to be on doesn't silently persist shadows-off. Turn
  the toggle off if you want to keep raster shadows. Like Solari's deferred renderer method, it is global rather than
  per-camera: while it is on, no camera renders shadow maps. The one exception
  is a light with a **Volumetric Light**: volumetric fog is drawn by Bevy's own
  pass and reads that light's shadow map to know where the fog is in shade, so
  it keeps it. Without it the fog was lit everywhere, indoors included, and from
  outside a building the interior vanished behind a bright haze.
- **Measure before tuning.** The [Ray Tracing panel](#ray-tracing-panel) shows
  each Solari pass's GPU time and the rays it traces; use it, or the debugger's
  render diagnostics, rather than inferring from the frame counter. The
  Graphics Quality tier also gates the fullscreen passes that stack on top of
  Solari. Solari's time spans are `solari_lighting/presample_light_tiles`,
  `world_cache`, `initial` (the new samples and paths), `spatial_and_shade`
  (ReSTIR's spatial reuse and shading), the denoiser's `denoise_temporal`,
  `denoise_atrous` and `denoise_composite`, and `diagnostics` while the panel
  measures.

## Diagnosing a dark or wrongly-shadowed scene

The plugin logs ray-tracing coverage whenever the tallies change:

```
[solari] ray-tracing scene: 1482 meshes mirrored, 6 skipped, 34 excluded (hidden / off-layer)
```

- **A low `mirrored` count** with a high `skipped` count means an under-populated
  BVH — Solari can only light what it can trace, so the view goes nearly black.
  Check the geometry against the table above.
- **`(N via stripped copies)`** counts meshes that needed a ray-tracing-only
  duplicate because of extra vertex attributes. A large number here is normal for
  imported scenes; it only costs memory.
- **A rising `excluded` count** is usually the editor doing offscreen work; the
  asset browser generating model thumbnails is the common one. That is the
  intended behaviour, not a problem — those models are excluded precisely so they
  don't shadow your scene.
- **Ghosting or a stale dark patch after a big change** is the temporal estimate
  re-converging. The inspector's **Reset Temporal History** button clears it.
- **The sun resets the history on its own.** Solari caches the light it has
  already computed, and that cache does not age stale light out on any useful
  timescale, so moving the sun below the horizon used to leave the scene lit by
  sunlight that was gone. Now any large change to a directional light (about
  three degrees of rotation, a fifth of its illuminance, or switching on or off)
  clears the history, the same as the button. Smaller changes converge on their
  own, so dragging the sun resets every few degrees, not every frame.

If the acceleration structure ends up completely empty, the plugin warns instead:

```
[solari] ray-tracing scene is EMPTY (0 skipped, 29 excluded) - every opaque
surface will render black.
```

This one is worth recognising on sight, because it does not look like "missing
GI" — it looks like a broken scene. With no acceleration structure Bevy never
builds the scene bind group, so the `solari_lighting` pass returns early; and
because a Solari camera also carries `SkipDeferredLighting`, **nothing else
lights the G-buffer**. The result is every opaque surface rendering pure black
while forward-path geometry — blended foliage, glass, the sky — still looks
perfectly lit. Trees and bushes standing in front of solid black buildings is the
signature.

## Ray Tracing panel

**Debug > Ray Tracing** shows what Solari is doing, so its quality can be judged
by measurement rather than by eye alone. It works on the focused viewport
while Solari Ray-Traced GI is on.

**Show** replaces the viewport's image with a view of Solari's internals:

| View | What it shows |
|---|---|
| Raw \| Denoised | Solari's raw estimate left of a divider (drag **Divider**), the denoised result right of it. |
| History length | Frames of history behind each pixel, blue for none to red for the denoiser's maximum of 32. Blue patches are where the image is still noisy. |
| Rejected history | Red where the denoiser threw its history away this frame: a surface newly seen, or one that changed. |
| Noise estimate | The denoiser's own reading of each pixel's noise, relative to its brightness, red at four times the light itself. It decides how hard each pixel is blurred. |
| ReSTIR confidence | How many samples each pixel's ReSTIR reservoir has gathered from past frames and neighbours. Blank on Quality: Low, which has no ReSTIR. |
| World cache | The cached light that bounce rays read. Blocky where its cells are large, which is far from the camera. |
| Not in ray-traced scene | Magenta where what was drawn is missing from, or different in, the ray-traced scene: a mesh left out, or an alpha-tested leaf traced as a solid card. Such surfaces cast and catch light wrongly. |
| Path-traced reference | The reference image (below). |
| Error vs reference | How far the live image is from the reference, blue for a match to red for 50% or more. |

DLSS steps aside while a view is shown, since it would otherwise take the
visualization for the image, so the built-in denoiser runs.

**Reference.** With **Accumulate** on, a conventional path tracer converges in
the background beside the live image, one sample per pixel per frame. Each path
starts from the surface drawn at its pixel, as the live image's lighting does,
so the point and spot lights' traced stand-ins and alpha-masked leaves look the
same in both; from there it is the same algorithm as Solari's offline path
tracer, checked against it by `renzora_solari`'s GPU tests. It leaves out glass
and other see-through surfaces, which ordinary rays pass through. It starts over
when the camera moves, and
**Start over** restarts it after a change the camera cannot see, such as a light
being moved. **Error vs reference** compares 3x3 neighbourhoods of the two
images, so the reference's own early noise counts for little; it falls as the
reference converges and settles at the live image's real error.

**Figures**, measured every frame while the panel is visible:

- **Rays**: lighting rays per frame and per pixel, split into first paths,
  ReSTIR's reuse and the world cache's update, each with how many were shadow
  rays, and a rolling graph.
- **Denoiser**: average history, the share of pixels under 4 frames and the
  share rejected this frame, and grain before and after denoising (each pixel's
  difference from its neighbours, relative to their brightness). A still camera
  should reach a full 32 frames with almost nothing rejected. With DLSS
  denoising, these describe the built-in denoiser, which is not running.
- **ReSTIR**: average reservoir confidence.
- **Brightness**: histograms of the exposed image before and after denoising,
  half a stop per bar. Denoising should narrow the spread.
- **Ray-traced scene**: the mesh tallies from the coverage log, the share of
  pixels the traced scene does not match, and the world cache's cell count.
- **GPU time** of each pass, including the glass and see-through pass.

Measuring costs GPU time (the `diagnostics` pass), so it stops when the panel is
hidden; a view or an accumulating reference carries on. While the panel
measures or the reference accumulates, the viewport renders every frame rather
than idling.

## Denoiser

Solari's raw estimate is grainy: each pixel is a handful of random light paths.
Renzora runs its own denoiser over it, so the image is clean on **any GPU that
can ray trace** (NVIDIA RTX, AMD RX 6000 and later, Intel Arc), not only where
NVIDIA's DLSS Ray Reconstruction runs.

The Solari Ray-Traced GI inspector's **Denoiser** setting picks what removes
the noise:

- **DLSS**, the default: DLSS Ray Reconstruction where it can run, this
  denoiser everywhere else.
- **Built-in**: this denoiser, even where DLSS could run.
- **Off (raw)**: nothing, in any mode. The raw estimate is the thing to compare
  a denoiser against when judging it.

With **DLSS** chosen, the **DLSS Mode** row appears only while DLSS is actually
running; without it, this denoiser is in use. See [DLSS](./dlss.md). A scene saved with the older separate **Denoiser** and
**DLSS** toggles loads with the matching choice.

How it works, in order:

1. **Split and demodulate.** The lighting passes write diffuse and specular
   lighting separately, each divided by its material factor (the diffuse colour,
   and the specular reflectance for the view direction). Filtering lighting
   rather than colour keeps texture detail sharp, and filtering the lobes apart
   keeps reflections from being blurred as much as a matte wall can be.
2. **Temporal accumulation.** Last frame's result is reprojected through the
   motion vectors and blended with the new sample. History is thrown away where
   the surface changed (a different plane or normal, as at the edge of something
   that moved), and each pixel keeps its own history length: up to 32 frames for
   diffuse light, down to 4 for mirror-like reflections while the camera
   moves, which would otherwise smear. A reflection keeps the full 32 frames
   while the direction it is seen from barely changes, so a glossy floor
   settles smooth in a still view. At a silhouette, where the camera's
   sub-pixel jitter keeps changing which surface a pixel sees, the history is
   looked for one pixel further before it is given up. While the camera moves, last frame's result is
   read between pixels with a sharpening (Catmull-Rom) filter rather than a
   bilinear one, which would soften it a little every frame. Bright samples are
   kept, not clamped: in indirect lighting they are real light, and clamping
   them left indirectly lit surfaces about a tenth too dark.
3. **Edge-aware blur.** Five passes of an a-trous wavelet filter, at spacings of
   1 to 16 pixels, that stop at surface planes, normals and brightness changes.
   How far a pixel is blurred follows how noisy its accumulated average still
   is, so a freshly revealed area is blurred hard and a converged one is left
   alone, keeping sharp sun-shadow edges and thin shadows. Mirror-like surfaces
   are not blurred at all. A pixel far brighter than every neighbour (a
   firefly, such as a sun glint off a glossy floor) is shown no brighter than
   its brightest neighbour allows, while its history keeps the true value.
4. **Composite.** The material factors are multiplied back in and emissive is
   added.

It is written from the published research (SVGF and the
edge-avoiding a-trous filter); no code comes from any vendor's denoiser SDK.

Its quality is measured rather than judged by eye. `renzora_solari`'s
`denoise_quality` test renders a sunlit room on a ray-tracing GPU with the
denoiser, without it, and with Solari's offline path tracer run to convergence,
and scores each image against that reference for grain, error and flicker,
standing still and while the camera moves. On a flat ceiling at viewport size
the denoised image is as smooth as the converged reference, about thirty times
smoother than the raw estimate. It is opt-in like the engine's other GPU tests:

```sh
RENZORA_GPU_TESTS=1 cargo test --profile dist -p renzora_solari --test denoise_quality -- --nocapture
```

`RENZORA_GPU_TEST_OUT=<dir>` writes the images, and
`RENZORA_SOLARI_TEST_SIZE=1280x1024` runs at a real viewport's size. Built with
`--features dlss` (and the NGX libraries on `PATH`), it also checks that
switching DLSS off hands the image back to this denoiser.

The `denoiser_benchmark` test is what changes to the denoiser are judged by. It
places cameras at random through the room from a fixed seed, so the denoiser is
not tuned to one view, runs the editor's setup (TAA on) standing still and
moving, and scores each against its own converged reference: overall error,
error on edges and detail (found from the reference, wherever they fall on
screen), grain on flat surfaces, flicker, and the denoiser's history figures.
Errors compare 3x3 neighbourhoods and leave out the worst 0.1% of pixels, so
the reference's own leftover noise does not decide the result. A camera that
lands inside furniture is skipped. `RENZORA_BENCH_POSE=<n>` runs one camera
(each renderer holds GPU memory until its process ends, so beside a running
editor one per process is safest), `RENZORA_BENCH_RAW=1` scores the raw
estimate, and `RENZORA_BENCH_DLSS=1` scores DLSS Ray Reconstruction, the
comparison the built-in denoiser is aimed at:

| Five cameras, mean | Error, still | Detail, still | Error, moving | Detail, moving | Grain | Flicker |
|---|---|---|---|---|---|---|
| Built-in denoiser | 0.022 | 0.059 | 0.021 | 0.075 | 0.036 | 0.0007 |
| DLSS Ray Reconstruction | 0.026 | 0.056 | 0.018 | 0.050 | 0.045 | 0.0016 |

The room's floor is glossy with a normal map and a roughness map, and its back
wall bumpy, because that texture detail in the lighting is where denoisers
differ most. The built-in denoiser's remaining gap is that detail while the
camera moves: resampling the history every frame softens it, where DLSS keeps
it.

The editor's viewport renders only for a short while after each change, then
holds its last image. While this denoiser runs it is given 48 frames rather
than the usual 20, enough to reach its full history; stopping sooner froze a
picture that was still visibly grainy.

Known limits of this first version: lighting changes catch up over up to 32
frames (Solari's own history resets on big sun changes), sharp reflections are
reprojected with their surface's motion rather than the reflected object's, and
ReSTIR's habit of sharing samples between neighbours can leave faint low-frequency
blotches.

## Renzora's copy of bevy_solari

The engine builds its own copy of `bevy_solari` (`crates/bevy_solari`), patched
over the one Bevy depends on. Besides the [denoiser](#denoiser) (`denoise.wesl`,
`denoise.rs`, and the lighting split it reads) and the diagnostics behind the
[Ray Tracing panel](#ray-tracing-panel) (`debug.wesl`, `debug_layout.wesl`,
`debug.rs`, and a per-thread ray count in the traced scene's `trace_ray` and
`trace_visibility`) and the [see-through
pass](#glass-and-other-see-through-surfaces) (`transparent.wesl`, the
per-instance ray masks, and each material's alpha and blend mode), it differs
from upstream in four places, each marked
`RENZORA PATCH` in the source:

- **Roughness is floored at 0.089**, as Bevy's raster renderer floors it. Upstream
  shaded a perfectly smooth surface with the specular maths degenerate, so mirror
  pixels (the puddles in a wet street's glossiness map) rendered black.
- **A visible emissive surface is not scaled by exposure**, matching Bevy's
  default for raster and deferred rendering. Upstream scaled it, so neon signs
  and bulbs came out roughly a thousand times dimmer with Solari on than off at a
  daylight exposure, and never reached the bloom threshold. Emissive surfaces
  still light the scene through Solari, exposed like any other light.
- **Rays leave a surface two pixels' width off what was drawn**
  (`gpixel_resolve` in `gbuffer_utils.wesl`). [Virtual
  geometry](virtual-geometry.md) draws each mesh at a simplified level up to a
  pixel away from the full detail that is traced, so a ray leaving the drawn
  surface could start underneath the traced one and be shadowed by it: black
  patches that grew with distance and at grazing angles. The lift is measured
  in pixels at the surface's depth, so it is millimetres up close and stays
  under what the eye can see at any distance. Upstream used a fixed 1 mm.
  (Virtual geometry now draws its finest level while Solari is on, so the two
  agree wherever that level is loaded; the lift still covers the moments one
  is streaming in.)
- **DLSS's guide textures follow the DLSS toggle every frame**
  (`sync_dlss_rr_textures` in `prepare.rs`). Upstream made them only when the
  view's other buffers were rebuilt, on a size change, and never removed them.
  Switching DLSS off at the same size (always, under DLAA) left them in place,
  which reads as "DLSS is denoising": the built-in denoiser stood aside, DLSS no
  longer ran, and the raw, grainy image reached the screen.

## Limitations

- RT-capable GPU + Vulkan/DX12/Metal backend only (never GL/web/Android).
- Ray hits are shaded from `StandardMaterial` data only. A node-graph material
  is traced with its `StandardMaterial` approximation, so anything procedural in
  it (noise, UV maths, a roughness worked out by a maths node) is missing from
  its reflections and bounce light. A hand-written custom material with no
  `StandardMaterial` behind it is not traced at all.
- Non-triangle-list or non-indexed geometry is not lit by Solari. A mesh missing
  normals, UVs or tangents is traced through a copy with them made up (smooth
  normals, zero UVs, tangents from the normals), which is right for an untextured
  surface such as glass or a clear coat.
- **No ambient light, and no native point/spot lights.** Solari lights from
  directional lights, emissive meshes and the [sky](#sky-light); point and spot
  lights work solely through [light proxies](#light-proxies), and `AmbientLight`
  not at all (see [Lights Solari can see](#lights-solari-can-see)). The image is
  higher-contrast than the forward pipeline, most visibly with the sun near the
  horizon (little reaches street level) or directly overhead (vertical facades
  get no direct light at all).
- **DLSS needs the SDK at build time.** NVIDIA's DLSS Ray Reconstruction is used
  only in a build made where the DLSS SDK is installed, on an RTX GPU, on the
  Vulkan renderer; everywhere else the [denoiser](#denoiser) runs and Solari
  traces at the viewport's full resolution. See [DLSS](./dlss.md).
- **Cutout geometry over-shadows.** `Mask` materials are traced as solid quads
  (Solari has no alpha-tested any-hit shading), so foliage cards cast fuller
  shadows than they should.
- **Glass has no refraction, and draws at most four layers deep.**
  See [Glass and other see-through surfaces](#glass-and-other-see-through-surfaces).

## Note on installed plugins

Enabling the `bevy_solari` Bevy feature recompiles the shared `bevy_dylib`. That
moves the content-hash stamp every installed plugin records, so the editor
rebuilds all of them on the next launch. Nothing breaks and nothing needs
re-pinning; the first start after the change just takes longer.

If an editor still has the old marketplace copy installed (`plugins/solari`
beside the binary), delete that folder. Left in place it would load a second
`SolariPlugin` on top of the built-in one.
