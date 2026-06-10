# Lumen GI Plan

As-built reference for `renzora_lumen`, the global-illumination distribution plugin — a voxel radiance clipmap, a voxel-cone diffuse tracer, and a screen-space reflection pyramid that ship today, plus the screen-space SSGI tier it bundles from `renzora_rt`.

> This file started life as a phased "Lumen-inspired" *plan*. Most of it shipped, but the **SDF architecture was abandoned** during implementation (no mesh `.msdf` bake, no global SDF clipmap) and replaced by **CPU geometry voxelization**. The sections below describe what is actually in the code. The historical phase plan is preserved in the [Appendix: abandoned designs](#appendix-abandoned-designs).

## What ships today

Two crates make up the GI stack:

- **`renzora_lumen`** — the GI distribution plugin. It ships as a `cdylib` dlopen plugin (dropped in the engine's `plugins/` directory and loaded at startup by `dynamic_plugin_loader`), self-registered with `renzora::add!(LumenPlugin)`. It is **not** statically linked into the host or editor — that would double-register through `add!`.
- **`renzora_rt`** — a **single-pass**, depth+normal-aware SSGI node. Despite the "rt" name there is no ray tracing here; it is the `ScreenSpace` GI tier. It is a **library linked into `renzora_lumen`** (never a standalone plugin and never statically linked into the host), so `RtLighting` has exactly one definition on both sides of the dlopen boundary.

`LumenPlugin` bundles `renzora_rt::RtPlugin` plus all of the Lumen passes in one dll:

```rust
// crates/renzora_lumen/src/lib.rs (abridged)
impl Plugin for LumenPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(renzora_rt::RtPlugin); // the ScreenSpace tier

        app.register_type::<LumenLighting>();
        app.add_systems(Update, (sync_lumen_lighting, cleanup_lumen_lighting));
        app.add_plugins(ExtractComponentPlugin::<LumenLighting>::default());

        app.add_plugins(VoxelCachePlugin);       // 4-cascade voxel radiance clipmap
        app.add_plugins(VoxelDownsamplePlugin);   // mip pyramid for the clipmap
        app.add_plugins(GeometryVoxelizePlugin);  // runtime CPU voxelization
        app.add_plugins(LumenTracePlugin);        // voxel-cone diffuse + inlined temporal
        app.add_plugins(ScreenReflectionPlugin);  // half-res SSR trace
        app.add_plugins(ScreenReflectionBlurPlugin);
        app.add_plugins(ScreenReflectionResolvePlugin);

        #[cfg(feature = "editor")]
        {
            app.init_resource::<renzora::LumenDiagState>();
            app.add_systems(Update, editor::update_lumen_diag_state);
            editor::register_inspectors(app);
        }
    }
}

renzora::add!(LumenPlugin); // Runtime scope — runs in the editor viewport and shipped games
```

> The plugin registers at **Runtime** scope, so it runs in the editor viewport *and* in exported games. The `editor` cargo feature (default-on) only adds the inspectors and the diagnostics producer; those registrations are harmless no-ops in a shipped game.

## Quality tiers

`LumenLighting.quality` selects a tier. The sync system (`sync_lumen_lighting` → `apply_quality`) translates the tier into the engine-level components on each active camera.

| Tier | What runs | Notes |
|---|---|---|
| `Off` | nothing | Strips `RtLighting`; voxel injection/trace inactive. |
| `ScreenSpace` (default) | `renzora_rt` SSGI | Inserts `RtLighting` + `RtLightingExternallyManaged` on the camera so `renzora_rt`'s own sync leaves it alone. |
| `SdfLow` | voxel-cone diffuse trace (2 cones, 20 steps) | Strips `RtLighting`; runs the voxel pipeline (`LumenTracePlugin`) at `quality_tier = 0`. |
| `SdfHigh` | voxel-cone diffuse trace (4 cones, 32 steps) | Same pipeline at `quality_tier = 1`. |
| `Hwrt` | **placeholder** — see below | Strips `RtLighting`. No hardware-RT backend exists. |

> ⚠️ **`Hwrt` is not implemented.** wgpu ray tracing is not enabled — `platform_wgpu_settings()` in `crates/renzora_runtime/src/lib.rs` only requests `POLYGON_MODE_LINE`, and `bevy_solari` is not wired in. There is no BVH/ray-gen path. As currently wired, selecting `Hwrt` does **not** give ray tracing: it is grouped with `SdfLow`/`SdfHigh` in the voxel-injection predicate and falls through the trace at `quality_tier = 0`, i.e. it behaves like `SdfLow`. Treat it as reserved.

> ℹ️ The in-source doc comments at `crates/renzora/src/gi.rs:79` and `crates/renzora_lumen/src/lib.rs:14` still say *"Phase 1 implements only Off and ScreenSpace; higher tiers render the same as Off."* That comment is **stale** — `SdfLow`/`SdfHigh` drive the voxel-cone trace today.

## Settings

All GI settings live in the **shared contract** `crates/renzora/src/gi.rs`, so the GI plugin's render systems, the editor inspectors, `renzora_level_presets`, and the debugger's Lumen panel share one `TypeId` across the dlopen boundary.

```rust
// crates/renzora/src/gi.rs
pub enum LumenQuality { Off, ScreenSpace, SdfLow, SdfHigh, Hwrt } // default: ScreenSpace
pub enum LumenDebug   { None, IndirectOnly, VoxelCache }          // default: None

pub struct LumenLighting {
    pub quality: LumenQuality,
    pub intensity: f32,
    /// Multiplier on the specular voxel-cone trace contribution.
    pub specular_intensity: f32,
    pub debug: LumenDebug,
}
// Default: quality = ScreenSpace, intensity = 0.4, specular_intensity = 1.0, debug = None

pub enum RtDebugMode { Composite, IndirectOnly }
pub struct RtLighting { pub enabled: bool, pub intensity: f32, pub debug: RtDebugMode }
// Default: enabled = true, intensity = 1.0, debug = Composite
```

| Field | Type | Meaning |
|---|---|---|
| `quality` | `LumenQuality` | Tier selector (see table above). |
| `intensity` | `f32` | Indirect-diffuse multiplier (feeds SSGI intensity *or* the voxel-cone `TraceConfig.intensity`). |
| `specular_intensity` | `f32` | Multiplier on the voxel-cone **specular** trace (`TraceConfig.specular_intensity`); `0` disables specular. Forced to `0` in the `IndirectOnly` debug view. |
| `debug` | `LumenDebug` | `None` / `IndirectOnly` (show indirect only) / `VoxelCache` (splat the voxel cache to screen, independent of quality). |

`LumenLighting` is authored on a **non-camera** entity (typically the "World Environment" entity), not on the camera directly. `EffectRouting` (`crates/renzora/src/core/mod.rs:777`) maps source entities onto the active cameras; `sync_lumen_lighting` mirrors the chosen settings onto each routed camera every frame.

```rust
// Author GI on the World Environment entity; routing pushes it to the cameras.
commands.entity(world_env).insert(LumenLighting {
    quality: LumenQuality::SdfHigh,
    intensity: 0.6,
    specular_intensity: 1.0,
    debug: LumenDebug::None,
});
```

There is **no** `settings.rs`/`extract.rs`/`prepare.rs`/`node.rs` quartet in `renzora_lumen` (the old plan called for that). Settings come from `gi.rs`; per-pass GPU config is `TraceConfig` inside `lumen_trace.rs`.

## Pipeline

Per active camera, with `SdfLow`/`SdfHigh`:

1. **Voxel radiance clipmap** is cleared/scrolled and injected with direct light.
2. **CPU geometry voxelization** contributes scene-surface samples into the clipmap.
3. **Mip downsample** builds the clipmap pyramid.
4. **Voxel-cone diffuse trace** gathers indirect light, with inlined temporal accumulation and a sky-cubemap fallback on cone miss.
5. **Screen-space reflection pyramid** supplies specular, which the trace pass reads back when compositing.

`renzora_rt`'s SSGI node (`ScreenSpace` tier) is a separate render-graph node (`RtLabel`) slotted between `Node3d::EndMainPass` and `Node3d::Tonemapping`, independent of the Lumen labels.

### Voxel radiance clipmap — `voxel_cache.rs`

A camera-centered, voxel-snapped clipmap stored as a single 3D texture with cascades stacked along Z:

- `CASCADE_COUNT = 4`, `VOXEL_RES = 64` → a 64³ `Rgba16Float` grid per cascade.
- Cascade world extents are `32 m / 64 m / 128 m / 256 m` (≈128 m of reach around the camera). The cone trace tests cascades inner-out and takes the first hit.
- A separate `u32` accumulation buffer backs the temporal EMA blend (≈8 MB radiance + ≈20 MB accumulation).
- Shaders: `voxel_clear.wgsl`, `voxel_inject.wgsl`, `voxel_resolve.wgsl`, `voxel_debug.wgsl`.

### CPU geometry voxelization — `geometry_voxelize.rs`

Instead of an offline mesh-SDF bake, scene geometry is contributed at runtime by sampling `Assets<Mesh>` on the CPU:

- `MeshVoxelSamples` holds a per-instance `Vec<Vec3>` of local-space surface sample points, baked from triangle data (per-instance so albedo overrides work cleanly), re-baked when the mesh/instance changes.
- A per-frame **bake throttle** caps how many meshes are voxelized per frame; samples are re-flattened into a fresh GPU buffer each frame and injected via `voxel_geo_inject.wgsl`.
- `LumenBakeStats` surface in the debugger's Lumen panel (bakes-this-frame, last/avg/max bake duration, totals).

### Mip downsample — `voxel_downsample.rs`

Builds the clipmap mip pyramid (`voxel_downsample.wgsl`). Each cascade keeps 64/32/16/8 voxels along Z across mips 0..3 (powers of two), so the 2× box filter never reads across a cascade boundary. Registers after the voxel resolve so mip 0 is ready before mips 1..N are generated. Coarser mips are what the diffuse cones sample as they widen with distance.

### Voxel-cone diffuse trace — `lumen_trace.rs` / `lumen_trace.wgsl`

The visible GI pass. Inputs: depth, normals, the voxel clipmap pyramid, the (optional) deferred G-buffer, and the resolved reflection buffer.

- **Voxel-cone diffuse**: per pixel, traces a set of cones through the clipmap. Cone count / step budget come from `quality_tier` (`SdfLow` = 2 cones / 20 steps, `SdfHigh` = 4 cones / 32 steps).
- **Inlined temporal**: motion-vector reprojection + EMA blend are done **inside this shader** — there are no separate `temporal_denoise.wgsl`/`spatial_denoise.wgsl` files (the old plan's "shared denoise library" was never built).
- **Sky-cubemap fallback**: a cone that leaves the clipmap or exhausts its step budget with remaining alpha samples the camera's prefiltered sky cubemap in the cone direction. `LumenSkyCubemap` is (re)attached from the camera's `EnvironmentMapLight` by `sync_lumen_sky_cubemap`; `TraceConfig.sky_intensity` rides along with `EnvironmentMapLight.intensity` (so it tapers to zero at night). This gives upward-facing surfaces ambient sky bounce when no voxel content is available.
- **Specular**: a voxel-cone specular trace, scaled by `specular_intensity`, reads the resolved screen-space reflection buffer.
- Output is `Rgba16Float`; `TraceConfig` is the per-frame uniform (`intensity`, `frame_count`, `debug_mode`, `quality_tier`, `sky_intensity`, `use_albedo_modulation`, `specular_intensity`).

### Screen-space reflection pyramid — `screen_reflection{,_blur,_resolve}.rs`

A dedicated half-resolution SSR pipeline (three stages):

1. **`screen_reflection.rs`** — half-res world-space ray march → reflection color + validity (`screen_reflection.wgsl`).
2. **`screen_reflection_blur.rs`** — promotes the half-res result into a mip pyramid (5 levels; coarser mips ≈ rougher reflections) (`screen_reflection_blur.wgsl`).
3. **`screen_reflection_resolve.rs`** — bilateral-upsamples the pyramid; `lumen_trace` reads this buffer and picks the mip from each pixel's roughness-derived `mip_level` (`screen_reflection_resolve.wgsl`).

> Reflections are screen-space, not a voxel/SDF reflection trace. The original plan's `lumen_reflections.wgsl` (SDF/voxel-cone reflections) was superseded by this pyramid.

## Tier → component sync

```rust
// crates/renzora_lumen/src/lib.rs::apply_quality (abridged)
commands.entity(target).try_insert((settings.clone(), RtLightingExternallyManaged));
match settings.quality {
    LumenQuality::ScreenSpace => {
        // delegate to renzora_rt SSGI
        commands.entity(target).try_insert(RtLighting {
            enabled: true,
            intensity: settings.intensity,
            debug: match settings.debug {
                LumenDebug::IndirectOnly => RtDebugMode::IndirectOnly,
                _ => RtDebugMode::Composite,
            },
        });
    }
    // SdfLow / SdfHigh handled by the voxel-cone trace (reads quality off the
    // mirrored LumenLighting). Off / Hwrt strip SSGI too.
    LumenQuality::Off | LumenQuality::SdfLow | LumenQuality::SdfHigh | LumenQuality::Hwrt => {
        commands.entity(target).remove::<RtLighting>();
    }
}
```

`RtLightingExternallyManaged` is the handshake between the two crates: when Lumen owns a camera it sets this marker, and `renzora_rt`'s `sync_rt_lighting` skips any camera that carries it (otherwise RT would clobber what Lumen writes every frame). `cleanup_lumen_lighting` removes `LumenLighting`/`RtLighting`/`RtLightingExternallyManaged` together when the source component goes away.

## Diagnostics

Under the `editor` feature, `LumenPlugin` produces `renzora::LumenDiagState` (a plain-primitive snapshot, so it crosses the dlopen boundary cleanly) for the debugger's **Lumen** panel:

- `cameras: Vec<LumenCameraEntry>` — per-camera `inject_active` / `debug_active` flags.
- `mesh_voxel_samples_entities` — how many entities currently have baked `MeshVoxelSamples`.
- `has_sky_cubemap` — whether the sky-cubemap fallback is bound.
- `bake: LumenBakeSnapshot` — CPU voxelization throttle stats (last/avg/max bake duration, bakes last frame, totals, per-frame budget).

`LumenDebug::VoxelCache` splats the voxel cache to screen (works at any quality so you can preview the cache); `LumenDebug::IndirectOnly` shows only the indirect contribution.

## Critical files

| Purpose | Path |
|---|---|
| GI settings contract (`LumenLighting`, `RtLighting`, enums, `LumenDiagState`) | `crates/renzora/src/gi.rs` |
| Plugin entry + tier sync + `add!` | `crates/renzora_lumen/src/lib.rs` |
| Voxel radiance clipmap (4 cascades) | `crates/renzora_lumen/src/voxel_cache.rs` (`voxel_clear/inject/resolve/debug.wgsl`) |
| Runtime CPU geometry voxelization | `crates/renzora_lumen/src/geometry_voxelize.rs` (`voxel_geo_inject.wgsl`) |
| Voxel mip downsample | `crates/renzora_lumen/src/voxel_downsample.rs` (`voxel_downsample.wgsl`) |
| Voxel-cone diffuse trace (+ inlined temporal, sky fallback) | `crates/renzora_lumen/src/lumen_trace.rs`, `lumen_trace.wgsl` |
| Screen-space reflection pyramid | `crates/renzora_lumen/src/screen_reflection{,_blur,_resolve}.rs` (+ `.wgsl`) |
| `ScreenSpace` SSGI tier | `crates/renzora_rt/src/{lib,node,prepare}.rs`, `ssgi.wgsl` |
| Editor inspectors + diagnostics producer | `crates/renzora_lumen/src/editor.rs` |
| `EffectRouting` (source → camera) | `crates/renzora/src/core/mod.rs:777` |
| wgpu features (HWRT gate; only `POLYGON_MODE_LINE`) | `crates/renzora_runtime/src/lib.rs` (`platform_wgpu_settings`) |
| Engine bootstrap (note: Lumen is **not** registered here) | `crates/renzora_runtime/src/lib.rs:112` (`init_app`) |

## Outstanding work

- **HWRT backend.** The only genuinely unbuilt tier. Requires enabling wgpu ray-tracing features in `platform_wgpu_settings()` (or wiring `bevy_solari`), a BLAS/TLAS build, and a ray-gen path, with runtime fallback to the voxel-cone trace when the adapter lacks RT support. Until then `LumenQuality::Hwrt` is a placeholder.

Everything else from the original plan that is still relevant (voxel cache, CPU voxelization, downsample, voxel-cone diffuse + temporal, sky fallback, screen-space reflections, debug views, diagnostics) is implemented.

## Appendix: abandoned designs

The original plan was a **Lumen-inspired, SDF-based** pipeline. These pieces were designed but **never built** (or were superseded); none of the files below exist:

| Abandoned design | What replaced it |
|---|---|
| Mesh-SDF bake at import time, `.msdf` sidecars, `MeshSdfLoader`, `sdf/` module, `bake.rs`, `loader.rs`, `bin/bake_sdf.rs` | Runtime **CPU geometry voxelization** (`geometry_voxelize.rs`) directly into the voxel clipmap. |
| Global SDF clipmap (`global_sdf.rs`, `global_sdf_compose.wgsl`, `sdf_instance_hash.wgsl`, `sdf_trace.wgsl`, `sdf_common.wgsl`) — a ray-marchable world-space SDF volume | The **voxel radiance clipmap** (`voxel_cache.rs`) is the only world-space structure; cones march voxels, not an SDF. |
| Emissive injection (`voxel_emissive_inject.wgsl`, screen-projected emissive) | Not built — there is no emissive-injection path in the crate. |
| SDF/voxel-cone reflection path (`lumen_reflections.wgsl`) | The half-res **screen-space reflection pyramid** (`screen_reflection*.rs`). |
| Separate denoise library (`temporal_denoise.wgsl`, `spatial_denoise.wgsl`, A-Trous) | Temporal accumulation is **inlined** in `lumen_trace.wgsl`; there is no separate denoise stage. |

> Historical naming note: the SSGI tier (`renzora_rt`) was once a 9-pass screen-space pipeline. That is gone — it is now a single-pass SSGI node. The "rt" name is historical and does not imply ray tracing.
