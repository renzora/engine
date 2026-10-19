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

  Geometry that still can't qualify — no UVs, not an indexed triangle list, or
  degenerate UVs that defeat tangent generation — is marked skipped so it isn't
  re-checked every frame, rather than crashing the builder.
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
  | Opaque alpha mode, **or emissive** | `bevy_solari` has no alpha handling at all, so a blended material is a solid wall to every ray. `Opaque`, `Mask` and `AlphaToCoverage` participate; `Blend`, `Premultiplied`, `Add` and `Multiply` are held back — *unless* the material is emissive. An emissive mesh is an area light only while it is a live instance, so excluding a blended one would delete a light rather than merely stop it occluding, and neon tubes and lamp glass are exactly that combination. |

  All four are re-checked every frame, so unhiding an object or changing its
  alpha mode puts it back into (or takes it out of) the ray-traced scene
  immediately.
- **Idle.** When no camera has Solari enabled, the ray-tracing mirror is dropped
  so BLAS resources are freed.

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

The world cache that stores bounce light scales with the preset too.

**ReSTIR is the setting that matters most.** It reuses light samples across
neighbouring pixels and earlier frames, and in Renzora it is the only thing
cleaning up Solari's noise (see [Limitations](#limitations)). `Low` turns it
off, which is why `Low` is the grainiest preset and not just the cheapest one.

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
  Console.
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

Solari is substantially more expensive than the raster pipeline, and
most of that cost is inherent to `bevy_solari` today rather than to how Renzora
drives it. Per frame it rebuilds the whole TLAS (`AccelerationStructureUpdateMode::Build`,
not a refit), clones every `StandardMaterial` into the render world, and rebuilds
every storage buffer and the scene bind group — then runs the world-cache,
ReSTIR DI, ReSTIR GI and specular passes on top.

Two things are worth knowing:

- **Shadow maps are suppressed while Solari is on** (`Suppress Shadow Maps` in
  the inspector, on by default). A Solari camera carries `SkipDeferredLighting`,
  so the standard lighting pass — the only consumer of a shadow map — never
  runs. Without suppression every directional cascade and every point-light
  cubemap is rendered in full and then discarded, which in a scene with a lot of
  lamps is a serious amount of wasted GPU time. Bevy's own guidance is the same:
  set `shadow_maps_enabled: false` on all lights while Solari is active.

  The suppression is applied in the **render world**, to the extracted lights.
  Your `PointLight` / `DirectionalLight` components are never written, so a scene
  saved while Solari happens to be on doesn't silently persist shadows-off. Turn
  the toggle off if you want to keep raster shadows. Like Solari's deferred renderer method, it is global rather than
  per-camera: while it is on, no camera renders shadow maps.
- **Measure before tuning.** Use the debugger's render diagnostics for real GPU
  timings rather than inferring from the frame counter; the Graphics Quality
  tier also gates the fullscreen passes that stack on top of Solari.

## Diagnosing a dark or wrongly-shadowed scene

The plugin logs ray-tracing coverage whenever the tallies change:

```
[solari] ray-tracing scene: 1482 meshes mirrored, 6 skipped, 34 excluded (hidden / off-layer / blended)
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

## Denoiser

Solari's raw estimate is grainy: each pixel is a handful of random light paths.
Renzora runs its own denoiser over it, so the image is clean on **any GPU that
can ray trace** (NVIDIA RTX, AMD RX 6000 and later, Intel Arc), not only where
NVIDIA's DLSS Ray Reconstruction runs. It is on by default; the **Denoiser**
toggle in the Solari Ray-Traced GI inspector turns it off to show the raw
estimate, which is the thing to compare against when judging it.

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
   diffuse light, down to 4 for mirror-like reflections, which would otherwise
   smear as the camera moves. A sample far brighter than its history (a
   "firefly") is clamped.
3. **Edge-aware blur.** Five passes of an a-trous wavelet filter, at spacings of
   1 to 16 pixels, that stop at surface planes, normals and brightness changes.
   How far a pixel is blurred follows how noisy it is, so a freshly revealed
   area is blurred hard and a converged one is left alone. Mirror-like surfaces
   are not blurred at all. The first pass's result becomes next frame's history.
4. **Composite.** The material factors are multiplied back in and emissive is
   added.

It is written from the published research (SVGF and the
edge-avoiding a-trous filter); no code comes from any vendor's denoiser SDK.

Known limits of this first version: lighting changes catch up over up to 32
frames (Solari's own history resets on big sun changes), sharp reflections are
reprojected with their surface's motion rather than the reflected object's, and
ReSTIR's habit of sharing samples between neighbours can leave faint low-frequency
blotches.

## Renzora's copy of bevy_solari

The engine builds its own copy of `bevy_solari` (`crates/bevy_solari`), patched
over the one Bevy depends on. Besides the [denoiser](#denoiser) (`denoise.wesl`,
`denoise.rs`, and the lighting split it reads), it differs from upstream in two
places, each marked `RENZORA PATCH` in the source:

- **Roughness is floored at 0.089**, as Bevy's raster renderer floors it. Upstream
  shaded a perfectly smooth surface with the specular maths degenerate, so mirror
  pixels (the puddles in a wet street's glossiness map) rendered black.
- **A visible emissive surface is not scaled by exposure**, matching Bevy's
  default for raster and deferred rendering. Upstream scaled it, so neon signs
  and bulbs came out roughly a thousand times dimmer with Solari on than off at a
  daylight exposure, and never reached the bloom threshold. Emissive surfaces
  still light the scene through Solari, exposed like any other light.

## Limitations

- RT-capable GPU + Vulkan/DX12/Metal backend only (never GL/web/Android).
- Ray hits are shaded from `StandardMaterial` data only. A node-graph material
  is traced with its `StandardMaterial` approximation, so anything procedural in
  it (noise, UV maths, a roughness worked out by a maths node) is missing from
  its reflections and bounce light. A hand-written custom material with no
  `StandardMaterial` behind it is not traced at all.
- Meshes without UVs, or with non-triangle-list/non-indexed geometry, are not lit
  by Solari.
- **No ambient light, and no native point/spot lights.** Solari lights from
  directional lights, emissive meshes and the [sky](#sky-light); point and spot
  lights work solely through [light proxies](#light-proxies), and `AmbientLight`
  not at all (see [Lights Solari can see](#lights-solari-can-see)). The image is
  higher-contrast than the forward pipeline, most visibly with the sun near the
  horizon (little reaches street level) or directly overhead (vertical facades
  get no direct light at all).
- **No DLSS.** Renzora does not build NVIDIA's DLSS Ray Reconstruction; the
  [denoiser](#denoiser) does its job on every vendor's hardware. There is
  therefore no upscaling either: Solari traces at the viewport's full
  resolution.
- **Cutout geometry over-shadows.** `Mask` materials are traced as solid quads
  (Solari has no alpha-tested any-hit shading), so foliage cards cast fuller
  shadows than they should.
- **Transparent materials don't render while Solari is on.** The global renderer
  method is deferred, which can't draw blended materials, so glass and
  transmissive surfaces read as fully transparent until Solari is switched off.

## Note on installed plugins

Enabling the `bevy_solari` Bevy feature recompiles the shared `bevy_dylib`. That
moves the content-hash stamp every installed plugin records, so the editor
rebuilds all of them on the next launch. Nothing breaks and nothing needs
re-pinning; the first start after the change just takes longer.

If an editor still has the old marketplace copy installed (`plugins/solari`
beside the binary), delete that folder. Left in place it would load a second
`SolariPlugin` on top of the built-in one.
