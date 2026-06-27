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
| **Unified post-process** | ~53 effect crates (`renzora_ascii`, `renzora_vignette`, `renzora_crt`, …) | A fullscreen fragment pass inside the single `UnifiedPostProcessNode`. Built with `#[post_process]` / `PostProcessEffect`. |
| **Bevy built-in wrappers** | `renzora_bloom_effect`, `renzora_dof`, `renzora_ssao`, `renzora_ssr`, `renzora_motion_blur`, `renzora_auto_exposure`, `renzora_atmosphere`, `renzora_skybox`, `renzora_environment_map`, `renzora_forward_decal`, `renzora_distance_fog`, `renzora_volumetric_fog`, `renzora_antialiasing` | Author user-facing settings, then route a **stock Bevy component** onto the camera (`Bloom`, `DepthOfField`, `ScreenSpaceAmbientOcclusion`, `ScreenSpaceReflections`, `Atmosphere`, `Skybox`, `EnvironmentMapLight`, `ForwardDecal`, FXAA/SMAA/TAA/CAS, …). No custom WGSL pass of their own. |
| **Custom multi-pass render-graph crates** | `renzora_lumen` + `renzora_rt` (GI), `renzora_oit` (transparency); plus material/mesh sky & water (`renzora_clouds`, `renzora_night_stars`, `renzora_water`, `renzora_pool_water`, `renzora_lighting`) | Their own render-graph nodes/passes, outside the unified node. |

The first two families both get their settings onto the camera the same way — through `EffectRouting` (below). The third family wires up its own graph nodes.

For authoring a unified effect (the `#[post_process]` macro, the WGSL contract, and the hand-written trait path), see **[Post-Processing Effects](/docs/r1-alpha5/extending/post-processing)**. This page covers the pipeline-level picture.

### The unified post-process node

All unified effects share **one** render-graph node. Each effect registers a type-erased `TypedEffectHandler<T>` into a `PostProcessRegistry`, and a handler runs only when its settings component is present on the view:

```rust
// crates/renzora/src/postprocess.rs (abridged)
impl ViewNode for UnifiedPostProcessNode {
    type ViewQuery = (Entity, &'static ViewTarget);

    fn run(/* … */) -> Result<(), NodeRunError> {
        let registry = world.resource::<PostProcessRegistry>();
        for handler in &registry.handlers {
            handler.execute(world, render_context, view_target, entity)?; // no component → returns early
        }
        Ok(())
    }
}
```

This means **inactive effects cost nothing** — there is no per-effect node and nothing executes for effects you didn't add. Effects run in the order their `PostProcessPlugin::<T>` registered (i.e. plugin registration order, which you can nudge with the `add!` priority). The framework itself lives inside `renzora.dll` (`renzora::postprocess`, re-exported through the `renzora_postprocess` shim) so all effect crates share one registry and matching `TypeId`s across the dynamic-plugin boundary.

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

## Debugging the pipeline

`renzora_debugger` ships several render-focused editor panels:

- **Render Stats** / **Render Pipeline** — draw counts and per-pass / pipeline information.
- **Lumen** — the GI diagnostics view (`LumenDiagState`: bake timings, voxel-sample counts, sky-cubemap presence).
- **Culling Debug** / **Camera Debug** — frustum and camera state.
- **Material Resolver** — how a material graph resolved to its final shader.

Wireframe visualization relies on the `POLYGON_MODE_LINE` feature noted above (so it is unavailable on the GL backend and on the web build).

## What's next

- **[Post-Processing Effects](/docs/r1-alpha5/extending/post-processing)** — author a unified effect end to end (`#[post_process]`, the WGSL contract, hand-written effects).
- **[WGSL Shaders](/docs/r1-alpha5/rendering/shaders)** — writing shaders and materials for Renzora.
- **[Camera System](/docs/r1-alpha5/rendering/camera)** — cameras, viewports, and prepasses.
- **[Architecture](/docs/r1-alpha5/setup/architecture)** — where the render crates sit in the one-binary / plugin model.
</content>
</invoke>
