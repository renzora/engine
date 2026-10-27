# Render Pipeline

How Renzora turns a scene into a frame: Bevy's PBR/HDR core, three families of camera effects routed through one unified post-process node, the OIT render pass layered on top, and Solari for global illumination.

## How a frame renders

Renzora does **not** ship a custom deferred/G-buffer renderer. It renders with **Bevy 0.20's built-in PBR pipeline**: physically based (Cook-Torrance) materials, clustered lighting, and a **16-bit-float HDR** render target followed by a tonemapping pass. On top of that core, Renzora inserts its own render passes and a large family of effect plugins. Every shader in that pipeline is WESL, the shader language Bevy 0.20 compiles; shaders still written in the old `#import` dialect are translated as they load (see [Shaders](./shaders.md)).

Clustered lighting's GPU light lists start at 4096 Z slices and 262,144 light indices (`renzora_lighting`), four times Bevy's default, so a scene with dozens of point lights does not render a few frames of wrong lighting while Bevy grows them.

Each viewport camera is a `Camera3d`. Cameras are spawned with **`DepthPrepass` + `NormalPrepass`** attached (plus `MotionVectorPrepass` for temporal reprojection), because masked materials need normals in the prepass and screen-space effects read depth and normals.

> Bevy (0.19 and 0.20) specializes the prepass pipeline at first render and cannot grow a camera's prepass attachment list afterwards (doing so trips a wgpu validation crash). The depth + normal prepasses are therefore attached permanently at camera spawn (`renzora_engine::camera`, `renzora_viewport::play_mode`), not toggled per effect.

### What the engine adds to the `Core3d` schedule

Bevy renders a 3D view as systems in the render app's `Core3d` schedule, not as a graph of nodes. Renzora's passes join it through the `RenderPhase` system sets, which the framework anchors against Bevy's own passes:

```
… main 3D pass
    → RenderPhase::Gi        (linear HDR, pre-tonemap)
    → temporal AA
    → RenderPhase::HdrPost   (bloom-like HDR effects)
    → tonemapping
    → RenderPhase::LdrPost   (most post-process effects)
    → RenderPhase::Overlay → …
```

- A pass in `Gi` operates on the linear, lit HDR image before tone mapping and temporal AA.
- The **unified post-process pass** runs each effect in the phase it asks for, most of them after tonemapping, so they see the already-tonemapped image.

`WgpuSettings` requests `POLYGON_MODE_LINE` (skipped on the GL backend, never requested on web) — which is what enables wireframe debug views — and, **when the GPU supports it**, Bevy Solari's hardware ray-tracing features. The host probes the adapter once at startup (`raytracing_supported()`); on an RT-capable GPU it requests the ray-tracing features and records `renzora::GpuRaytracing { enabled: true }` so the optional `renzora_solari` plugin can activate. On a non-RT GPU nothing extra is requested and the engine boots unchanged. See [Solari ray-traced GI](./solari.md).

## Three families of camera effects

Not everything called a "camera effect" is the same kind of thing. There are three structural families:

| Family | Example crates | How it renders |
|---|---|---|
| **Shader-file effects** | [effect files](../extending/post-processing.md) on a Post Process component (`crt.wesl`, `sepia.wesl`, …), run by `renzora_postprocess` | Fullscreen fragment shaders, run in list order as one pass in `LdrPost`. A file declares its own sliders; nothing is compiled per effect, and an edit hot-reloads. |
| **Bevy built-in wrappers** | `renzora_bloom_effect`, `renzora_dof`, `renzora_ssao`, `renzora_ssr`, `renzora_vignette`, `renzora_motion_blur`, `renzora_auto_exposure`, `renzora_atmosphere`, `renzora_skybox`, `renzora_environment_map`, `renzora_forward_decal`, `renzora_distance_fog`, `renzora_volumetric_fog`, `renzora_antialiasing` | Author user-facing settings, then route a **stock Bevy component** onto the camera (`Bloom`, `DepthOfField`, `ScreenSpaceAmbientOcclusion`, `ScreenSpaceReflections`, `Atmosphere`, `Skybox`, `EnvironmentMapLight`, `ForwardDecal`, FXAA/SMAA/TAA/CAS, …). No custom WGSL pass of their own. |
| **Custom multi-pass render-graph crates** | `renzora_oit` (transparency); plus material/mesh sky & water (`renzora_clouds`, `renzora_night_stars`, [`renzora_water`](water.md), `renzora_pool_water`, `renzora_lighting`) | Their own render-graph nodes/passes, outside the unified node. `renzora_water` is the one that runs *before* the camera driver: its FFT wave simulation is view-independent, so it would be pure waste per view. |

The **wrappers** and the **Post Process** component get onto the camera through `EffectRouting` (below). The third family wires up its own graph nodes.

For writing an effect file (the header, the slider comments and the shader contract), see **[Post-Processing Effects](../extending/post-processing.md)**. This page covers the pipeline-level picture.

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

The framework lives inside `renzora.dll` (`renzora::postprocess`) so every pass shares one registry and matching `TypeId`s across the dlopen boundary. A plugin's pass registers through the same `add_render_pass` an engine one does, so the two sort against each other in one list.

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

The table is rebuilt every frame (by the viewport crate in the editor, by `renzora_engine` at runtime). Each effect plugin runs a small sync system that copies its component from a routed source onto the target camera and removes it again when the source disappears. The Bevy-wrapper family, OIT and Solari all consume the same table, so the editor inspector can drive viewport effects without you touching the camera entity.

## Global illumination: Solari

The engine's only global illumination is **Solari**: hardware ray-traced GI delivered by the optional **`renzora_solari`** plugin, which wraps Bevy Solari. It is authored with a `SolariGi` component on the **World Environment**, routed onto cameras through `EffectRouting`, and activates only on an RT-capable GPU; elsewhere the scene renders with direct lighting, IBL and SSAO as usual. See [Solari ray-traced GI](./solari.md).

## Reflection probes — parallax-corrected cubemaps

For local reflections that screen-space reflections can't supply (off-screen
geometry, interiors), the engine exposes Bevy's **parallax-corrected
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

### When environment maps are filtered

Bevy filters every `GeneratedEnvironmentMapLight` into its radiance and
irradiance maps on every frame, for every entity carrying one, with no way to
bake once. The GPU part is cheap; the render-thread part is not, because each
filter rebuilds its buffers, views and bind groups from scratch. With the sky
probe, the material preview and any reflection probes all present, that was the
largest single cost of an idle editor frame.

`renzora_environment_map` therefore filters a map only while its input is
changing, and freezes it otherwise. A map filters for a few frames after:

- the component is added or edited (new source, new intensity),
- its source image asset loads or changes,
- the camera carrying it turns active,
- for the sky probe, the sun, the atmosphere or the environment settings change.

A map on an inactive camera, or at zero intensity, never filters. A frozen map
keeps its last result, which for an unchanged input is the correct one. The
freeze happens in the render world, so the component itself stays on the camera
and nothing that reads or writes it needs to know.

## Planar reflections: `renzora_planar_reflection`

For a flat surface where the reflection is the point (a mirror, a window, a
polished floor, still water), a `PlanarReflection` component renders the scene
again from the viewing camera mirrored across the surface, through an oblique
near plane, and blends it over the surface's material. Unlike SSR and probes it
shows off-screen content, particles and transparent objects, at the cost of one
extra scene render per visible mirror. See
[Planar Reflections](./planar-reflections.md).

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

## Adding a render pass

If a unified post-process pass isn't enough (you need a compute pass, an extra render target, or to slot before tonemapping), add a render pass yourself. There is no render graph any more: a pass is a system in the `Core3d` schedule of the render app, ordered by system sets. Put it in the [`RenderPhase`](../extending/post-processing.md#render-phases) it belongs to and the framework anchors it for you. The pattern looks like this:

```rust
use bevy::core_pipeline::Core3d;
use bevy::render::renderer::{RenderContext, ViewQuery};
use bevy::render::view::ViewTarget;
use bevy::render::RenderApp;

fn my_pass(
    world: &World,
    // Views that lack any of these components are skipped.
    view: ViewQuery<(&'static ViewTarget, &'static MySettings)>,
    mut render_context: RenderContext,
) {
    let (view_target, settings) = view.into_inner();
    // ...record your passes on render_context...
}

// in Plugin::build:
if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
    render_app.add_systems(Core3d, my_pass.in_set(renzora::RenderPhase::Gi));
}
```

`MySettings` reaches the render world through `ExtractComponentPlugin`. Since Bevy 0.20 extraction names its target app: implement `ExtractComponent<RenderApp>` and `SyncComponent<RenderApp>`, or derive `ExtractComponent` with `#[extract_app(RenderApp)]`. Pipeline descriptors also gained a `constants` field (`constants: vec![]` when you override nothing), and a prepass depth texture is read with `ViewPrepassTextures::depth_only_view()`.

## Ambient occlusion — the SSAO section

SSAO is not a component you add. It is a **section of the one `WorldEnvironment`**, alongside Fog — select the World Environment entity and the Inspector shows an **SSAO** section whose header toggle is the effect's on/off switch. There is no trash button, because the section is intrinsic to the environment.

Turning it off removes `ScreenSpaceAmbientOcclusion` from the routed cameras outright, so "off" costs nothing (SSAO's pipeline key reads the component directly, in sync with its bind group, so unlike the resident-and-neutral effects this one is safe to toggle by presence).

The section's fields:

| Field | Effect |
|---|---|
| **Quality** | Sample-count preset — `Low` (4 spp), `Medium` (8), `High` (18, the default), `Ultra` (54), or `Custom`. Cost scales roughly with the sample count; noise falls as it rises. |
| **Object Thickness** | How thick, in world units, the engine assumes geometry is behind the depth buffer. A ray passing further behind a surface than this is treated as missing it rather than being occluded by it — raise it to darken thin geometry, lower it to thin out haloes behind foreground objects. Default `0.25`. |
| **Slice Count** | *Custom only.* Slices traced per pixel. More slices means less noise and a proportionally more expensive pass. |
| **Samples / Slice** | *Custom only.* Samples per slice side; 2 or 3 is the recommended range. |

The last two rows are hidden unless **Quality** is `Custom`, because every preset supplies its own pair and would override whatever you typed there.

```rust
// crates/renzora/src/world_environment.rs
WorldEnvironment {
    ssao: SsaoSection {
        enabled: true,
        quality: SsaoQuality::High,
        slice_count: 3,
        samples_per_slice_side: 3,
        constant_object_thickness: 0.25,
    },
    ..default()
}
```

Bevy 0.20 added a sample **radius** to SSAO; the section does not expose it yet, so it stays at Bevy's default and SSAO looks as it did on 0.19.

Values are clamped on the way to Bevy (`slice_count` 1–16, `samples_per_slice_side` 1–8, thickness ≥ 0), so a hand-edited scene file cannot produce a zero-sample pass or stall the GPU with a huge one. Scenes saved before these fields existed load with the defaults above rather than zeros.

## Graphics quality tiers

Most of the cost of an idle scene is **fullscreen, resolution-bound** work on the
active camera — the per-frame atmosphere→IBL cubemap bake, the volumetric
cloud raymarch, a raymarched sky, SSAO, the auto-exposure
histogram, bloom, and TAA — **not geometry**. That cost is scene-independent (one cube costs the same
as a full level) and scales with pixel count, so it dominates on weak GPUs and
high-DPI displays where the physical framebuffer is 2–4× the logical size. The
quality tier is the single switch that trades those passes for frame rate:

| Tier | IBL bake | Shadows | Clouds | Atmosphere | SSAO | Auto-exp | Bloom | TAA |
|---|---|---|---|---|---|---|---|---|
| **High** | 256² | 2048² | full march | Raymarched | on | on | on | on |
| **Medium** *(default)* | **128²** | **1024²** | **capped steps** | **LookupTexture** | **off** | on | on | on |
| **Low** | **64²** | **512²** | **off** | LookupTexture | off | **off** | **off** | **off** |

(Shadows = per-cascade `DirectionalLightShadowMap` resolution; each of the up-to-4
cascades clears a depth target that size every frame regardless of geometry, so
halving it quarters the per-cascade depth bandwidth.)

`Medium` is the shipping default: it sheds the heaviest fullscreen passes
(SSAO, the ~40×-cheaper lookup sky instead of the raymarch, and a
16× smaller IBL bake) while preserving the tonemapped look, so the engine runs
acceptably on older / integrated GPUs. `Low` is a compatibility floor.

SSAO is `High`-only rather than sitting with bloom/TAA at
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
  unconditionally the way it did before r1-alpha8.

```toml
[rendering]
mode = "deferred"
graphics_quality = "Medium"   # Low | Medium | High
render_scale = 1.0            # 3D resolution as a fraction of the logical window
virtual_geometry = true       # see Virtual Geometry
virtual_geometry_budget_mb = 512   # GPU memory streamed virtual geometry may use
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
so nothing grows a camera's bind-group layout after first render: bloom / TAA /
auto-exposure / SSAO remove the routed component; the
atmosphere switches `rendering_method` (a field, not an add/remove); clouds cap
their march step counts, then despawn their separate dome entity outright; and
the IBL probe's *face size* is chosen at camera spawn. The spawn-locked **prepass bundle** and the atmosphere/IBL *components*
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
- **Culling Debug** / **Camera Debug** — frustum and camera state.
- **Material Resolver** — how a material graph resolved to its final shader.

Wireframe visualization relies on the `POLYGON_MODE_LINE` feature noted above (so it is unavailable on the GL backend and on the web build).

## What's next

- **[Post-Processing Effects](../extending/post-processing.md)** — author an effect end to end: the three files, the WGSL contract, and what stays in-tree.
- **[Native Plugins](../extending/native-plugins.md)** — how an effect plugin is built and installed.
- **[WGSL Shaders](./shaders.md)** — writing shaders and materials for Renzora.
- **[Camera System](./camera.md)** — cameras, viewports, and prepasses.
- **[Architecture](../setup/architecture.md)** — where the render crates sit in the one-binary / plugin model.
</content>
</invoke>
