# Render Pipeline

How Renzora turns a scene into a frame: Bevy's PBR/HDR core, three families of camera effects routed through one unified post-process node, and the Lumen GI and OIT render-graph crates layered on top.

## How a frame renders

Renzora does **not** ship a custom deferred/G-buffer renderer. It renders with **Bevy 0.19's built-in PBR pipeline** — physically based (Cook-Torrance) materials, clustered lighting, and a **16-bit-float HDR** render target followed by a tonemapping pass. On top of that core, Renzora inserts its own render-graph nodes and a large family of effect plugins.

Each viewport camera is a `Camera3d`. Cameras are spawned with **`DepthPrepass` + `NormalPrepass`** attached, because the screen-space GI pass needs depth and normals.

> Bevy 0.19 specializes the prepass pipeline at first render and cannot grow a camera's prepass attachment list afterwards (doing so trips a wgpu validation crash). The depth + normal prepasses are therefore attached permanently at camera spawn (`renzora_engine::camera`, `renzora_viewport::play_mode`), not toggled per effect.

### What the engine adds to the `Core3d` graph

The two Renzora-owned insertions into Bevy's `Core3d` sub-graph are:

```
… → Node3d::EndMainPass
        → RtLabel                 (renzora_rt SSGI — linear HDR, pre-tonemap)
    → Node3d::Tonemapping
        → UnifiedPostProcess      (every active post-process effect)
    → Node3d::EndMainPassPostProcessing → …
```

- **SSGI** (the `renzora_rt` node, label `RtLabel`) runs **between `EndMainPass` and `Tonemapping`**, so it operates on the linear, lit HDR image before tone mapping.
- The **unified post-process node** runs **between `Tonemapping` and `EndMainPassPostProcessing`**, so every fullscreen effect sees the already-tonemapped image.

`WgpuSettings` requests `POLYGON_MODE_LINE` (skipped on the GL backend, never requested on web) — which is what enables wireframe debug views — and, **when the GPU supports it**, Bevy Solari's hardware ray-tracing features. The host probes the adapter once at startup (`raytracing_supported()`); on an RT-capable GPU it requests the ray-tracing features and records `renzora::GpuRaytracing { enabled: true }` so the optional `renzora_solari` plugin can activate. On a non-RT GPU nothing extra is requested and the engine boots unchanged. See [Solari ray-traced GI](./solari.md).

## Three families of camera effects

Not everything called a "camera effect" is the same kind of thing. There are three structural families:

| Family | Example crates | How it renders |
|---|---|---|
| **Unified post-process** | 53 [standalone plugins](../extending/standalone-plugins.md) under `plugins/` (`ascii`, `crt`, `sepia`, …) | A fullscreen fragment pass, registered through the C ABI with `add_post_process`. These link no Bevy and hot-reload, shader included. |
| **Bevy built-in wrappers** | `renzora_bloom_effect`, `renzora_dof`, `renzora_ssao`, `renzora_ssr`, `renzora_vignette`, `renzora_motion_blur`, `renzora_auto_exposure`, `renzora_atmosphere`, `renzora_skybox`, `renzora_environment_map`, `renzora_forward_decal`, `renzora_distance_fog`, `renzora_volumetric_fog`, `renzora_antialiasing` | Author user-facing settings, then route a **stock Bevy component** onto the camera (`Bloom`, `DepthOfField`, `ScreenSpaceAmbientOcclusion`, `ScreenSpaceReflections`, `Atmosphere`, `Skybox`, `EnvironmentMapLight`, `ForwardDecal`, FXAA/SMAA/TAA/CAS, …). No custom WGSL pass of their own. |
| **Custom multi-pass render-graph crates** | `renzora_lumen` + `renzora_rt` (GI), `renzora_oit` (transparency); plus material/mesh sky & water (`renzora_clouds`, `renzora_night_stars`, [`renzora_water`](water.md), `renzora_pool_water`, `renzora_lighting`) | Their own render-graph nodes/passes, outside the unified node. `renzora_water` is the one that runs *before* the camera driver — its FFT wave simulation is view-independent, so it would be pure waste per view. |

The **wrappers** get their settings onto the camera through `EffectRouting` (below). The third family wires up its own graph nodes. Plugin effects need neither: their settings component sits on any entity and the bridge uploads its bytes each frame.

For authoring a unified effect (the three files, the WGSL contract, and where the line falls between a plugin effect and an in-tree one), see **[Post-Processing Effects](../extending/post-processing.md)**. This page covers the pipeline-level picture.

### Render composition — one registry, four phases

Effects do not each get a render-graph node. They register a type-erased pass into a single `RenderComposition` resource, tagged with a **phase** and an **order**, and the registry keeps itself sorted by `(phase, order)`:

```rust
// crates/renzora/src/postprocess.rs (abridged)
#[derive(Resource, Default)]
pub struct RenderComposition {
    passes: Vec<RenderPassEntry>,   // kept sorted by (phase, order)
}
```

Four dispatcher systems drain it, one per phase, each pinned into `Core3d` relative to Bevy's own post-process anchors:

| Phase | Runs | Image |
|---|---|---|
| `Gi` | `EarlyPostProcess`, before temporal AA | HDR |
| `HdrPost` | `EarlyPostProcess`, after temporal AA | HDR |
| `LdrPost` | `PostProcess`, after tonemapping, before FXAA/SMAA | LDR |
| `Overlay` | `PostProcess`, after FXAA/SMAA and after `LdrPost` | final |

This file is the **only** place that imports `tonemapping`, `temporal_anti_alias`, `fxaa` and `smaa`. An effect never orders against them; it names a phase and the framework positions it. Ordering against an anchor that isn't in the schedule — a 2D lean export with no anti-alias plugin — is a harmless no-op, which is why the anti-alias anchors are `render_3d`-gated and the phases still work without them.

A pass runs only when its settings component is present, so **inactive effects cost nothing** — no pipeline bind, no pass. Within a phase, `order` decides; there is no per-effect priority API and the `add!` priority does not affect render order.

The framework lives inside `renzora.dll` (`renzora::postprocess`, re-exported through the `renzora_postprocess` shim) so every in-tree effect shares one registry and matching `TypeId`s across the dlopen boundary. Standalone plugin effects reach the same registry through `renzora_postprocess::plugin_bridge`, which turns an `add_post_process` call from the C ABI into a `RenderPassEntry` — so a plugin effect and an engine pass sort against each other in one list.

## EffectRouting — getting settings onto the camera

You rarely attach effect components to the camera directly. Instead you author them on any entity (the inspector does this for you), and the engine proxies them onto the active cameras through the **`EffectRouting`** resource:

```rust
// crates/renzora/src/core/mod.rs
#[derive(Resource, Default, Debug)]
pub struct EffectRouting {
    /// (target_camera, [source_entities]) — for a given settings type the FIRST source that has it wins.
    pub routes: Vec<(Entity, Vec<Entity>)>,
}
```

The table is rebuilt every frame (by the viewport crate in the editor, by `renzora_engine` at runtime). Each effect plugin runs a small sync system that copies its component from a routed source onto the target camera and removes it again when the source disappears. The Bevy-wrapper family and the GI/OIT crates all consume the same table, so the editor inspector can drive viewport effects without you touching the camera entity.

## Global illumination — `renzora_lumen`

GI is delivered by **`renzora_lumen`**, a dlopen distribution plugin (`renzora::add!(LumenPlugin)`). It also statically links **`renzora_rt`** and installs `RtPlugin`, so the `RtLighting` type has a single definition shared across the main and render worlds.

The GI settings types — `RtLighting`, `LumenLighting`, `LumenQuality`, `LumenDebug`, `LumenDiagState` — live in the shared contract (`crates/renzora/src/gi.rs`) so editor inspectors, `renzora_level_presets`, and the debugger's Lumen panel all share one `TypeId` across the dlopen boundary. `LumenLighting` is authored on a non-camera entity (typically the **World Environment**) and is mutually exclusive with a hand-attached `RtLighting`:

```rust
// renzora::LumenLighting (defaults)
LumenLighting {
    quality: LumenQuality::ScreenSpace,
    intensity: 0.4,
    specular_intensity: 1.0,   // multiplier on the voxel-cone specular trace
    debug: LumenDebug::None,
}
```

### Quality tiers

`LumenLighting.quality` selects how indirect light is computed:

| `LumenQuality` | What runs | Status |
|---|---|---|
| `Off` | Strips `RtLighting`; no GI | Working |
| `ScreenSpace` | Delegates to `RtLighting` — the single-pass SSGI node in `renzora_rt` | Working (default) |
| `SdfLow` | Voxel-cone diffuse trace at low voxel-cache resolution (SSGI stripped) | Working |
| `SdfHigh` | Voxel-cone diffuse trace at full voxel-cache resolution | Working |
| `Hwrt` | Reserved Lumen HWRT tier | **Renders nothing** today |

> Lumen's own `Hwrt` tier parses and can be selected, but **produces no GI** — it is a placeholder for a future Lumen hardware backend. Treat it as equivalent to `Off`. Some in-source comments still read "only `Off` and `ScreenSpace` are implemented" — that is stale; `SdfLow`/`SdfHigh` are live.
>
> Hardware ray-traced GI **does** ship — as a separate backend, not a Lumen tier: the optional **`renzora_solari`** plugin wraps Bevy Solari. It is independent of `LumenLighting` (authored via its own `SolariGi` component) and the two are mutually exclusive per camera. See [Solari ray-traced GI](./solari.md).

### What `LumenPlugin` installs

Beyond the `ScreenSpace` SSGI backend (`RtPlugin`), the voxel/trace/reflection tiers are built from:

- `VoxelCachePlugin` — a 4-cascade voxel radiance clipmap.
- `VoxelDownsamplePlugin` — the mip pyramid over the voxel radiance texture.
- `GeometryVoxelizePlugin` — **runtime CPU voxelization** of scene meshes into the cache.
- `LumenTracePlugin` — voxel-cone diffuse GI with **inlined** temporal accumulation and a sky-cubemap fallback when a cone misses.
- `ScreenReflectionPlugin` + `ScreenReflectionBlurPlugin` + `ScreenReflectionResolvePlugin` — a three-stage half-res screen-space reflection pyramid (trace → blur → bilateral upsample).

`renzora_rt` itself is the cheap tier: a **single-pass**, depth+normal-aware SSGI node. (Despite the crate name, it is *not* the historical "9-pass ray-tracing beast" — that design is gone. It is a library linked into `renzora_lumen`, never registered as a standalone plugin.)

> The originally planned mesh-SDF architecture (`.msdf` bakes, a global SDF clipmap, emissive injection) was **abandoned** and replaced by the CPU geometry-voxelization path above. If you find references to `MeshSdfLoader`, `sdf/`, `bake.rs`, or `voxel_emissive_inject.wgsl`, they describe code that was never built.

`LumenDebug` offers `None`, `IndirectOnly` (show only the indirect contribution), and `VoxelCache` (visualize the radiance cache). The live bake stats feed `LumenDiagState`, which the debugger's Lumen panel renders.

## Reflection probes — parallax-corrected cubemaps

For local reflections that screen-space reflections can't supply (off-screen
geometry, interiors), the engine exposes Bevy 0.19's **parallax-corrected
reflection probes**. Add one from **Add Entity → Lighting → Reflection Probe**;
it spawns an entity with:

- `LightProbe` — marks the probe and sets edge `Falloff`. The probe's
  **Transform** (position + scale) is the box it influences.
- `ReflectionProbeSource` — the authored **Source (HDR / Cube)** + `Intensity`,
  edited in the inspector. An equirectangular `.exr`/`.hdr`/`.png` is reprojected
  into the power-of-two `Rgba16Float` cubemap bevy's filter requires (a
  `.ktx2`/`.dds` cube container is used directly). Only this component persists;
  the probe is inert until a source is set.
- `ParallaxCorrection` — `None` (treat the reflection as infinitely far),
  `Auto` (correct against the probe's Transform box — the default), or `Custom`
  half-extents in probe space. Editing the extents switches to `Custom`.

`renzora_environment_map` watches `ReflectionProbeSource`, loads/reprojects the
image, and **only then** attaches the GPU `GeneratedEnvironmentMapLight` with the
finished cube. This ordering matters: bevy's environment-map filter runs the
moment that component exists and asserts a power-of-two cube, so attaching it
with an unset (1×1) handle would spam GPU validation errors — the engine sidesteps
that by adding it only once a valid cube is ready.

The editor draws the correction box as a teal wireframe gizmo (bright when the
probe is selected) so you can size it against the room. The authored source path
persists with the scene and the cube is regenerated on load.

## Order-independent transparency — `renzora_oit`

`renzora_oit` wraps Bevy's `OrderIndependentTransparencySettings` and is routed onto cameras via `EffectRouting` like any other effect:

```rust
// renzora_oit::OitSettings (defaults)
OitSettings {
    layer_count: 8,        // OIT depth layers
    alpha_threshold: 0.0,
    enabled: true,
}
```

When enabled it inserts `OrderIndependentTransparencySettings` on the camera and **forces `Msaa::Off`** (OIT and MSAA are incompatible); when disabled or its source is removed, it strips the component again.

## Adding a render-graph node

If a unified post-process pass isn't enough (you need a compute pass, an extra render target, or to slot before tonemapping), add a Bevy render-graph node yourself. This is the same pattern `renzora_rt` uses:

```rust
use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::render::render_graph::{RenderGraphExt, RenderLabel, ViewNodeRunner};
use bevy::render::RenderApp;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct MyLabel;

// in Plugin::build:
if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
    render_app
        .add_render_graph_node::<ViewNodeRunner<MyNode>>(Core3d, MyLabel)
        // slot the node into the graph between two existing nodes:
        .add_render_graph_edges(Core3d, (Node3d::EndMainPass, MyLabel, Node3d::Tonemapping));
}
```

> Edges are resolved when you add them — Bevy does no lazy lookup. If your node references another crate's label (e.g. `LumenTraceLabel`), that plugin must register **before** yours, or the edge panics with "node does not exist". Register custom GI/reflection nodes in dependency order.

## Graphics quality tiers

Most of the cost of an idle scene is **fullscreen, resolution-bound** work on the
active camera — the per-frame atmosphere→IBL cubemap bake, the procedural cloud
dome, a raymarched sky, screen-space GI, SSAO, the auto-exposure histogram, bloom,
and TAA — **not geometry**. That cost is scene-independent (one cube costs the same
as a full level) and scales with pixel count, so it dominates on weak GPUs and
high-DPI displays where the physical framebuffer is 2–4× the logical size. The
quality tier is the single switch that trades those passes for frame rate:

| Tier | IBL bake | Shadows | Clouds | Atmosphere | Screen-space GI | SSAO | Auto-exp | Bloom | TAA |
|---|---|---|---|---|---|---|---|---|---|
| **High** | 256² | 2048² | on | Raymarched | on | on | on | on | on |
| **Medium** *(default)* | **128²** | **1024²** | on | **LookupTexture** | **off** | **off** | on | on | on |
| **Low** | **64²** | **512²** | **off** | LookupTexture | off | off | **off** | **off** | **off** |

(Shadows = per-cascade `DirectionalLightShadowMap` resolution; each of the up-to-4
cascades clears a depth target that size every frame regardless of geometry, so
halving it quarters the per-cascade depth bandwidth.)

`Medium` is the shipping default: it sheds the heaviest fullscreen passes
(screen-space GI, SSAO, the ~40×-cheaper lookup sky instead of the raymarch, and a
16× smaller IBL bake) while preserving the tonemapped look, so the engine runs
acceptably on older / integrated GPUs. `Low` is a compatibility floor.

SSAO sits with screen-space GI at `High`-only rather than with bloom/TAA at
`Medium` because its three full-res compute passes are exactly the cost class
`Medium` exists to shed. Profiling put it **second only to the deferred prepass**
among GPU passes (0.46 ms of a 2.63 ms GPU frame on a discrete card, proportionally
far worse on the integrated GPUs `Low` exists for), and it was previously ungated,
so choosing `Low` explicitly for frame rate still paid for it. Gating it took the
measured GPU total at `Low` from 2.870 ms to 1.798 ms — **−37%**.

**The tier applies in both the editor and shipped games**, from two sources that
write one shared `ResolvedGraphicsQuality` resource:

- **Editor** — **Settings → Viewport → Performance → Graphics Quality**, stored per
  project on `ViewportSettings.graphics_quality` in `project.toml`'s editor-only
  `[editor]` section. Enforced on the viewport cameras by
  `renzora_level_presets::graphics_quality` (Editor scope).
- **Shipped game** — the `[rendering] graphics_quality` key (`"Low"` / `"Medium"` /
  `"High"`), which lives outside the `[editor]` block so export keeps it. The
  runtime resolves it and enforces it on the play camera in
  `renzora_engine::graphics_quality` (registered only when `!is_editor`). Without a
  key a game defaults to `Medium` — a game no longer runs the full stack
  unconditionally the way it did before r1-alpha7.

```toml
[rendering]
mode = "deferred"
graphics_quality = "Medium"   # Low | Medium | High
render_scale = 1.0            # 3D resolution as a fraction of the logical window
```

### The integrated-GPU hint

The tier defaults to `Medium`, and the users who most need `Low` are the least
likely to go looking for Settings → Viewport → Performance — the editor just feels
slow and there's no reason to suspect a setting. So on first launch, if the
adapter is integrated (`renzora::GpuIsIntegrated`, probed once from wgpu at
startup) and the tier isn't already `Low`, the editor raises a toast suggesting it.

It is deliberately a **hint, not an action**: nothing changes the tier for you. A
silently-applied override would be indistinguishable from the engine misbehaving,
and someone on integrated graphics may well have picked their tier on purpose.
There's no "already asked" flag on disk either, because the condition is
*self-clearing* — it only fires while the tier isn't `Low`, so acting on it
silences it permanently. Ignoring it costs one toast per launch, which is about the
right pressure for a hint you haven't acted on.

Mechanically the tier is a **ceiling, not an authority**: it only ever forces an
effect *off*, and remembers the last tier so raising it re-applies effects from
their untouched scene sources. Where a tier leaves an effect on, the inspector
still fully owns it. Every mutation is one a router already performs dynamically,
so nothing grows a camera's bind-group layout after first render: GI flips an
`enabled` flag; bloom / TAA / auto-exposure / SSAO remove the routed component; the
atmosphere switches `rendering_method` (a field, not an add/remove); clouds despawn
their separate dome entity; and the IBL probe's *face size* is chosen at camera
spawn. The spawn-locked **prepass bundle** and the atmosphere/IBL *components*
themselves stay resident at every tier (only their cheaper settings change) —
toggling those at runtime trips a wgpu validation crash.

### 3D render scale

Most of that fullscreen cost scales with **pixel count**, and on a high-DPI
display the pixel count is a trap: a window configured 1280×720 *logical* renders
at the **physical** framebuffer size — 1920×1080 at 150% scaling, **2.25× the
pixels** — for no benefit at the game's chosen resolution. The `[rendering]
render_scale` key renders the 3D scene at a fraction of the logical window and
upscales it to fill the window, with the **UI composited on top at native
(crisp) resolution**:

```toml
[rendering]
render_scale = 1.0   # default; 0.1 – 2.0
```

Because it's a fraction of the **logical** window, `render_scale = 1.0` renders at
the design resolution — which on a high-DPI display is fewer pixels than the
physical framebuffer, so **`1.0` already undoes the DPI pixel-bloat automatically**
with no per-machine value, and is a **zero-overhead no-op** on a 1.0-DPI display
(it renders straight to the window). Below `1.0` it doubles as a straight
performance slider (`0.5` = quarter the 3D pixels); at or above the display's DPI
factor it saturates to native — it never super-samples. It is **shipped-game only**
(the editor uses the per-camera **Render Resolution** Full/Half/Quarter), and it
stands down entirely while a non-`Disabled` `[viewport] stretch_mode` owns the
present pass.

### Vsync and measuring frame cost

A shipped game creates its window with vsync on, which caps the frame rate to the
monitor's refresh — so on a fast GPU the reported FPS is pinned at (say) 60 and
**hides the true per-frame cost**. To read real frametimes, uncap it with the
`[window] vsync` key (it lives outside `[editor]`, so export keeps it):

```toml
[window]
width = 1280
height = 720
vsync = false   # uncap the frame rate — useful for profiling
```

`apply_window_config` maps this to the window's `PresentMode` (`AutoVsync` when
true, `AutoNoVsync` when false). Defaults to `true`.

## Debugging the pipeline

`renzora_debugger` ships several render-focused editor panels:

- **Render Stats** — draw counts and per-pass information.
- **Lumen** — the GI diagnostics view (`LumenDiagState`: bake timings, voxel-sample counts, sky-cubemap presence).
- **Culling Debug** / **Camera Debug** — frustum and camera state.
- **Material Resolver** — how a material graph resolved to its final shader.

Wireframe visualization relies on the `POLYGON_MODE_LINE` feature noted above (so it is unavailable on the GL backend and on the web build).

## What's next

- **[Post-Processing Effects](../extending/post-processing.md)** — author an effect end to end: the three files, the WGSL contract, and what stays in-tree.
- **[Standalone Plugins](../extending/standalone-plugins.md)** — the C ABI the effect plugins are built on.
- **[WGSL Shaders](./shaders.md)** — writing shaders and materials for Renzora.
- **[Camera System](./camera.md)** — cameras, viewports, and prepasses.
- **[Architecture](../setup/architecture.md)** — where the render crates sit in the one-binary / plugin model.
</content>
</invoke>
