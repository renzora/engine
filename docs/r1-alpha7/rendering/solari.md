# Solari — hardware ray-traced global illumination

Solari is Renzora's **hardware ray-traced** GI backend. It wraps Bevy's
experimental `bevy_solari` (`SolariPlugins`) and ships as an optional drop-in
plugin, **`renzora_solari`**. Unlike Lumen (screen-space / voxel-cone GI), Solari
traces against a real BVH of the scene: fully dynamic direct **and** indirect
lighting from emissive meshes and directional lights, with no baking.

> **Experimental.** Solari requires a recent ray-tracing-capable GPU and is still
> evolving upstream. It is shipped as a separate, optional plugin precisely so it
> can be added or removed without touching the rest of the engine.

## How to enable

1. **Build the plugin.** `renzora_solari` is a `cdylib` distribution plugin. A
   normal `renzora build` / `renzora run` produces `renzora_solari.{dll,so,dylib}`
   alongside the other plugins.
2. **Drop it in `plugins/`.** Place the plugin next to the engine binary (the
   loader scans `<exe>/plugins/`). Present ⇒ Solari is available; delete it ⇒ it's
   gone. Nothing in the host references it.
3. **Run on an RT-capable GPU.** At startup the host probes the GPU adapter
   (`raytracing_supported()`); on a GPU that reports the ray-tracing wgpu features
   it requests them and sets `renzora::GpuRaytracing { enabled: true }`. If the
   GPU can't do ray tracing, the plugin logs a warning and stays **inert** — the
   engine still boots normally.
4. **Author `Solari Ray-Traced GI`.** Select the **World Environment** entity and
   add the *Solari Ray-Traced GI* component (Inspector → Add Component →
   Lighting). Toggle it on.

## Why a GPU capability flag (and not pure drop-in)

A plugin loaded from `plugins/` gets full ECS/`App` access in its `build()`, but
that runs **after** the `RenderDevice` is created. Ray-tracing wgpu features
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

This keeps the plugin a true drop-in for the *GI behaviour* while making the
underlying GPU capability a one-time, plugin-agnostic enablement.

## What the plugin does when active

- **Settings.** `SolariGi` has three fields: `enabled`; `suppress_shadow_maps`
  (**Suppress Shadow Maps**, on by default — see [Performance](#performance));
  and `light_proxies` (**Point/Spot Light Proxies**, on by default — see
  [Light proxies](#light-proxies)).
- **Camera.** `SolariGi` is routed from the World Environment onto each active
  camera via `EffectRouting` (the same path Lumen uses). On a routed camera the
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
  | `MeshMaterial3d<StandardMaterial>` | The only material type Solari's binder reads. |
  | Visible (`InheritedVisibility`) | An entity hidden with the hierarchy eye, or a drag-and-drop model ghost, must not cast ray-traced shadows. Frustum culling (`ViewVisibility`) is deliberately **not** consulted — off-screen geometry has to stay in the BVH, since shadowing and bounce from outside the view is the point of ray tracing. |
  | On render layer 0 | Layer 0 is the scene; layers 1+ are gizmos and the editor's offscreen rigs (asset-browser model thumbnails, the material viewer, the animation studio preview). Those live in the same `World`, at or beside the world origin, and would otherwise become invisible solid occluders standing in the middle of your level. |
  | Opaque alpha mode, **or emissive** | `bevy_solari` has no alpha handling at all, so a blended material is a solid wall to every ray. `Opaque`, `Mask` and `AlphaToCoverage` participate; `Blend`, `Premultiplied`, `Add` and `Multiply` are held back — *unless* the material is emissive. An emissive mesh is an area light only while it is a live instance, so excluding a blended one would delete a light rather than merely stop it occluding, and neon tubes and lamp glass are exactly that combination. |

  All four are re-checked every frame, so unhiding an object or changing its
  alpha mode puts it back into (or takes it out of) the ray-traced scene
  immediately.
- **Idle.** When no camera has Solari enabled, the ray-tracing mirror is dropped
  so BLAS resources are freed.

## Lights Solari can see

This is the single most surprising thing about Solari, and the usual reason a
scene comes out darker than the raster pipeline. `bevy_solari` 0.19 samples
**exactly two** kinds of light source:

| Source | Supported |
|---|---|
| `DirectionalLight` (the sun) | Yes |
| Emissive meshes | Yes — sampled as real area lights |
| `PointLight` | Not natively — Renzora stands one up as an emissive sphere (below) |
| `SpotLight` | Not natively — same, but the cone is lost |
| `AmbientLight` | **No**, and there is no workaround |
| Sky / atmosphere / environment map | **No.** There is no ambient term. |

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
- **Ambient and sky lighting cannot be represented this way at all.** That means
  `AmbientLight` *and* the environment map — including the procedural atmosphere
  the World Environment bakes into one by default. The only shape either could
  take in a traced scene is an enclosing emissive dome, which would then sit
  between every surface and the sun.

  **This is usually the single biggest reason a scene looks darker under
  Solari**, and it is invisible in the scene tree. An outdoor daylight scene
  draws a large share of its light from the sky: the baked atmosphere IBL is
  what fills in every surface not facing the sun. Losing it takes facades and
  shadowed ground close to black while the sunlit ground stays correct. The
  plugin warns when it finds an `EnvironmentMapLight` or `AmbientLight` on a
  Solari camera.

  Watch out for auto-exposure making this worse rather than better.
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
- There is no ambient or sky term, so everything in shadow sits lower than it
  did under the raster pipeline.
- Emissive surfaces still bloom normally — Solari adds `material.emissive` and
  applies view exposure exactly as the raster path does.

If you want the same look at a lower light level, drop `BloomSettings.threshold`
rather than raising light intensities.

## Performance

Solari is substantially more expensive than the raster pipeline or Lumen, and
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
  the toggle off if you want to keep raster shadows — for instance to compare
  against Lumen. Like Solari's deferred renderer method, it is global rather than
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

## Limitations

- RT-capable GPU + Vulkan/DX12/Metal backend only (never GL/web/Android).
- `StandardMaterial` only; custom-WGSL materials are not traced.
- Meshes without UVs, or with non-triangle-list/non-indexed geometry, are not lit
  by Solari.
- **No sky or ambient term, and no native point/spot lights.** Solari lights from
  directional lights and emissive meshes only; point and spot lights work solely
  through [light proxies](#light-proxies), and `AmbientLight` not at all — see
  [Lights Solari can see](#lights-solari-can-see). Surfaces facing away from the sun are lit by
  bounce alone, so the image is inherently darker and higher-contrast than the
  forward pipeline. This is most visible with the sun near the horizon (little
  reaches street level) or directly overhead (vertical facades get no direct
  light at all).
- **No denoiser.** Bevy's only Solari denoiser is DLSS Ray Reconstruction, which
  needs a compile-time cargo feature plus the NVIDIA DLSS SDK at build time and is
  not enabled in Renzora. Without it the image relies on Solari's own ReSTIR
  temporal accumulation and stays visibly noisy — give it a couple of seconds to
  converge after a change.
- **Cutout geometry over-shadows.** `Mask` materials are traced as solid quads
  (Solari has no alpha-tested any-hit shading), so foliage cards cast fuller
  shadows than they should.
- **Transparent materials don't render while Solari is on.** The global renderer
  method is deferred, which can't draw blended materials, so glass and
  transmissive surfaces read as fully transparent until Solari is switched off.
- Solari and Lumen are **mutually exclusive per camera** — don't author both
  `SolariGi` and `LumenLighting` on the same World Environment.

## Plugin ABI note

Enabling the `bevy_solari` Bevy feature recompiles the shared `bevy_dylib`, which
**moves the plugin ABI hash** (see [the plugin ABI section](../extending/plugins.md)
and `CLAUDE.md` §3). Every existing distribution plugin must be rebuilt against
the new dylib, and the pinned ABI hash is re-pinned to the new value after the
build.
