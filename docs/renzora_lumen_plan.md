# renzora_lumen — Lumen-inspired global illumination plugin

## Context

The engine currently has two tiers of indirect lighting:

- **`renzora_rt`** — despite its name, is a pure screen-space pipeline (Hi-Z + SSGI trace + screen-space radiance cache + SSR + SS shadows + A-Trous denoise, 9 compute passes between `Node3d::EndMainPass` and `Node3d::Tonemapping`).
- **`renzora_ssr`, `renzora_ssao`** — standard screen-space effects.

Bevy upstream has `bevy_solari` (hardware ray tracing, NVIDIA RTX in practice) but it is not wired into this codebase. wgpu ray tracing is not enabled — `platform_wgpu_settings()` in `crates/renzora_runtime/src/lib.rs:47-65` only sets `POLYGON_MODE_LINE`.

Screen-space alone hits the classic Lumen-era failure modes: off-screen geometry is invisible to GI (green couch behind you doesn't bleed onto the wall you're facing), no sky bounce at scale, reflections can't show anything behind the camera, disocclusion ghosting. Shipping a modern-looking renderer needs a world-space data structure.

The goal is a new standalone crate `renzora_lumen` that implements a **Lumen-inspired** pipeline — SDF-based, works on every GPU today, with an optional HWRT backend reserved for when wgpu ray tracing lands. `renzora_rt` stays as the cheap tier (mobile / low-end / perf mode). `renzora_lumen` becomes the default mid/high tier.

No Nanite equivalent exists; Lumen's surface-card system is out of scope. We replace cards with a **voxel radiance cache**, which is lower quality on thin geometry but tractable for a single implementer.

> **Status (as shipped):** The SDF architecture described below was **abandoned during implementation**. The actual `renzora_lumen` is **voxel-only**: a 4-cascade voxel radiance clipmap (`voxel_cache.rs`) populated by runtime **CPU voxelization** of scene geometry (`geometry_voxelize.rs`), cone-traced for diffuse GI (`lumen_trace.rs` / `lumen_trace.wgsl`), with a sky-cubemap fallback on cone miss and a separate screen-space reflection pyramid (`screen_reflection*.rs`). There is **no mesh-SDF bake, no `.msdf` loader, and no global SDF clipmap**. Phase headings below are kept for history; abandoned phases are marked inline.

---

---

## Architecture overview

Per-camera `LumenLighting` component with a `LumenQuality` enum: `Off / ScreenSpace / SdfLow / SdfHigh / Hwrt`. The render node slots into `Core3d` at the same position as `renzora_rt`'s node. `renzora_rt` and `renzora_lumen` are mutually exclusive on a camera (sync system enforces it).

Pipeline:

1. Mesh SDFs baked at import time as `.msdf` sidecars next to `.glb` files
2. Global SDF clipmap (4 cascades around camera) composed per-frame from mesh SDF instances
3. Voxel radiance cache (4-cascade 64³ RGBA16F clipmap) injected with direct light (and emissive) each frame
4. Lumen trace: try screen-space first, fall back to SDF march, sample voxel cache at hit
5. Diffuse integrator (probe-style) + temporal + A-Trous spatial denoise
6. Reflections via same trace infra
7. Composite into HDR before tonemapping

Follows `renzora_rt` conventions exactly: `Cargo.toml` shape, `lib.rs`/`extract.rs`/`prepare.rs`/`node.rs`/`settings.rs`/`shaders/` layout, `sync_lumen_lighting` using `renzora::EffectRouting`, `editor` feature gate with `inline_property` + `InspectorEntry` + phosphor icon.

---

## Phases

Each phase is independently shippable and produces a visible improvement.

### Phase 1 — Scaffold + Off/ScreenSpace delegation

**Goal:** Crate compiles, registers in runtime, inspector entry appears, `ScreenSpace` quality delegates to `renzora_rt` with zero regression.

**Deliverables:**
- `crates/renzora_lumen/Cargo.toml` (shape of `crates/renzora_rt/Cargo.toml`, add `renzora_rt` dep)
- `crates/renzora_lumen/src/lib.rs` — `LumenPlugin`, `sync_lumen_lighting`, `cleanup_lumen_lighting`, inspector entry
- `crates/renzora_lumen/src/settings.rs` — `LumenLighting` component, `LumenQuality` enum, `LumenPushConstants`
- `crates/renzora_lumen/src/{extract,prepare,node}.rs` — stubs
- `crates/renzora_lumen/src/shaders/{common,passthrough}.wgsl`
- Register in `Cargo.toml` workspace members (root) and `crates/renzora_runtime/src/lib.rs:112`

**Sync rule:** inserting `LumenLighting` removes `RtLighting` from target camera; reverse also true.

**User sees:** "Lumen GI" inspector entry with quality dropdown. `ScreenSpace` mode identical output to old `renzora_rt`. Zero perf regression.

### Phase 2 — Voxel radiance clipmap + direct-light injection

**Goal:** World-space voxel cache populated with direct light. No sampling yet — verified via debug view.

**Deliverables:**
- `crates/renzora_lumen/src/voxel_cache.rs` — `VoxelClipmap` render-world resource: 4 cascades × 64³ RGBA16F + R8 opacity + R8 age, camera-centered, voxel-snapped
- `shaders/voxel_clear.wgsl` — recycles voxels crossing cascade boundary
- `shaders/voxel_inject.wgsl` — loops clustered lights, accumulates with EMA decay
- `shaders/sdf_common.wgsl` (library) — voxel addressing, trilinear sample, cascade selection
- Debug view mode `VoxelCache` (splat voxels to screen)

**Budget:** ~10 MB VRAM, ~0.8 ms inject.

**User sees:** debug view shows voxelized direct lighting following the camera. No lighting change in final output yet.

### Phase 3 — Mesh SDF bake + `.msdf` asset loader — **ABANDONED**

> **Abandoned.** No `sdf/`, `mesh_sdf.rs`, `bake.rs`, or `.msdf` loader (`loader.rs`) was ever built. Instead of baking per-mesh SDFs offline, geometry is contributed at runtime via **CPU voxelization** of `Assets<Mesh>` directly into the voxel clipmap (`crates/renzora_lumen/src/geometry_voxelize.rs`). The remainder of this section is kept for historical context only.

**Goal:** Per-mesh SDFs generated offline/on-import, loaded via the 5-tier asset reader.

**Deliverables:**
- `crates/renzora_lumen/src/sdf/mesh_sdf.rs` — CPU jump-flood generator reading `Assets<Mesh>` (pattern from `crates/renzora_mesh_edit/src/systems.rs:28,59`)
- `crates/renzora_lumen/src/sdf/bake.rs` — async background bake via `Task<_>` when sidecar missing
- `crates/renzora_lumen/src/loader.rs` — `MeshSdfLoader` implementing `AssetLoader` (mirrors `crates/renzora_animation/src/loader.rs:50-135`), extension `"msdf"`
- `crates/renzora_lumen/src/sdf/format.rs` — header + R8_snorm 32³ (Low) / 64³ (High) volume + object AABB + world↔SDF matrix
- Optional `crates/renzora_lumen/src/bin/bake_sdf.rs` CLI for batch bake

**Budget:** 32 KB per mesh Low / 256 KB High. ~500 unique meshes ≈ 16–128 MB disk.

**User sees:** "Baking SDFs…" progress on first load; cached on disk after.

**Risks:** thin-mesh quality (fall back to brute force ≤ 100k tris). Static meshes only this phase.

### Phase 4 — Global SDF clipmap composition on GPU — **ABANDONED**

> **Abandoned.** No `sdf/global_sdf.rs`, `global_sdf_compose.wgsl`, `sdf_instance_hash.wgsl`, or `sdf_trace.wgsl` exists. There is no global SDF volume; the voxel clipmap (`voxel_cache.rs`) is the only world-space structure, fed by CPU voxelization (`geometry_voxelize.rs`). The remainder of this section is kept for historical context only.

**Goal:** Mesh SDFs composed into a 4-cascade global SDF volume that can be ray-marched.

**Deliverables:**
- `crates/renzora_lumen/src/sdf/global_sdf.rs` — `GlobalSdfClipmap`: 4 × 256³ R8_snorm (High) / 128³ (Low)
- `shaders/sdf_instance_hash.wgsl` — scatter SDF instances into a 3D hash grid per cascade
- `shaders/global_sdf_compose.wgsl` — per cell: min-blend nearby instance SDFs
- `shaders/sdf_trace.wgsl` (library) — sphere-trace utilities
- Extract: `ExtractedSdfInstances` (Vec<(transform, sdf_handle, aabb)>)
- Scroll strategy: only recompose cells that crossed the camera snap boundary

**Budget:** 64 MB High / 16 MB Low. ~1.5 ms compose when moving, <0.3 ms stationary.

**User sees:** debug view `GlobalSdf` shows screen-space slice. No lighting change yet.

**Deferred:** dynamic-mesh updates, terrain heightfield SDF (analytic plane fallback for now), skinned SDFs.

### Phase 5 — Voxel-cache cone-trace GI → first visible GI improvement

**Goal:** Cone-trace the voxel radiance clipmap from each surface to gather off-screen indirect light. **First phase where Lumen output visibly beats `renzora_rt`.**

**Deliverables:**
- `lumen_trace.wgsl` / `lumen_trace.rs` — voxel-cone tracer. Inputs: depth, normals, `VoxelCache`. (No SDF march and no Hi-Z screen-space fallback — the trace is voxel-cone only.)
- `LumenTraceResources` in `lumen_trace.rs` — output `Rgba16Float` at quarter/half/full-res by quality

**Budget:** half-res ~2.5 ms at 1080p on Steam Deck class. Full-res ~5 ms.

**User sees:** GI now extends past screen edges. Caves stay dark, rooms pick up off-screen color bleed, sky bounce works outdoors.

**Risks:** cascade-boundary light leaks (mitigate with cone angle that widens with hit distance); disocclusion halos (need Phase 6's temporal).

### Phase 6 — Diffuse integrator + temporal, ship-ready quality

**Goal:** Smooth, stable diffuse GI at shippable quality. First shippable tier.

**Deliverables:**
- `shaders/diffuse_integrator.wgsl` — probe-to-pixel resolve (depth/normal weighted gather)
- Duplicate `renzora_rt`'s `temporal_denoise.wgsl` and `spatial_denoise.wgsl` into `renzora_lumen/shaders/` (refactor to shared lib deferred to Phase 9)
- Motion-vector reprojection using `MotionVectorPrepass`, `DepthPrepassDoubleBuffer`, `PreviousViewUniforms`
- `reset: bool` flag on component (mirrors `RtLighting.reset`)
- `shaders/composite.wgsl` — Lumen-specific composite into HDR

**Budget:** +2 ms. Total Lumen path Medium ≈ 6–7 ms at 1080p.

**User sees:** GI stable in cutscenes, cuts snap cleanly. **This is the first ship-quality state.**

**Decision:** probe resolution 16 px Low/Med, 8 px High/Ultra.

### Phase 7 — Emissive injection + area lights — **ABANDONED**

> **Abandoned.** No `voxel_emissive_inject.wgsl` (or any emissive-injection path) was built. The remainder of this section is kept for historical context only.

**Goal:** Emissive materials light the environment (neon, lava, forge).

**Deliverables:**
- `shaders/voxel_emissive_inject.wgsl` — rasterize scene emissive into voxel grid at low res
- Alternative primary: screen-projected emissive from deferred GBuffer (cheap, visible-only)
- Recommend both: screen-space default, rasterized as `High+` opt-in

**Budget:** +0.4 ms.

**User sees:** TV glow on walls, bioluminescent plants, forge lighting the anvil.

### Phase 8 — Sky-cubemap fallback on cone miss

> **As shipped (commit `e72b7908`, "Lumen Phase 8: sky cubemap on cone miss").** Phase 8 did not become the planned reflection path. When a diffuse cone leaves the clipmap or exhausts its step budget with remaining alpha, `lumen_trace.wgsl` fills the unfilled portion by sampling the prefiltered **sky cubemap** in the cone direction (per-channel luminance clamp). This is what gives upward-facing surfaces ambient sky bounce when no voxel-cache content is available.
>
> **Reflections shipped later** as **Phase 10d** — a dedicated half-res **screen-space reflection pyramid** (`screen_reflection.rs`/`.wgsl`, `screen_reflection_blur.rs`/`.wgsl`, `screen_reflection_resolve.rs`/`.wgsl`; commit `f9515ee2`), not the SDF/voxel-cone reflection path described below.

**Goal (original plan, superseded):** Replace `renzora_ssr` and `renzora_rt`'s `ss_reflections` for Lumen cameras. Sharp → single combined trace; glossy → wider cone sampling voxel cache.

**Deliverables (original plan):**
- `shaders/lumen_reflections.wgsl` — roughness-dependent path, GGX importance sampling with blue noise from `renzora_bluenoise`
- Reuse A-Trous + temporal on the reflection slice
- Sync system disables `renzora_ssr` on Lumen-active cameras

**Budget:** ~1.5 ms Medium, ~3 ms Ultra.

**User sees:** reflections show off-screen geometry correctly (wet floor reflecting off-screen ceiling fan).

### Phase 9 — Presets, debug views, profiling HUD

**Goal:** Artist-facing polish.

**Deliverables:**
- `LumenQuality::apply_quality` sweeps all knobs
- `LumenDebug` enum: `None / VoxelCache / GlobalSdf / ProbeResolve / TraceMask`
- Inline egui pass-timing histogram (match `renzora_rt` inspector style)
- Sample scene + docs

> **Note:** The "refactor shared denoise shaders into a proper library" item was never done — there are no separate denoise shaders. Temporal accumulation (motion-vector reprojection + EMA blend) is **inlined directly in `lumen_trace.wgsl`**, so there is nothing to extract.

### Phase 10 (future) — HWRT backend

**Goal:** When wgpu RT is stable, `LumenQuality::Hwrt` replaces SDF tracer with real BVH rays; voxel cache + integrator + denoiser unchanged.

**Deliverables:**
- `hwrt` cargo feature enabling wgpu RT features in `platform_wgpu_settings()`
- `crates/renzora_lumen/src/hwrt/` — BLAS/TLAS build, ray-gen shader
- Runtime fallback to SDF if adapter doesn't support RT

---

## Cross-cutting decisions

- **Coexistence:** `LumenLighting` on a camera removes `RtLighting` (and disables `renzora_ssr`); reverse also true. Enforced in `sync_lumen_lighting`.
- **Required prepass components** inserted by sync (same as `renzora_rt`): `DepthPrepass`, `MotionVectorPrepass`, `DepthPrepassDoubleBuffer`, `CameraMainTextureUsages` with storage binding. Add `DeferredPrepass` for emissive/roughness access.
- **Skinned meshes:** excluded from SDF occlusion until post-Phase 10. They still receive GI.
- **Terrain:** analytic plane SDF fallback in Phase 4; heightfield-texture SDF path as Phase 4b if needed.
- **Async bake UX:** missing-SDF meshes contribute voxel-only occlusion (blurrier) until bake finishes — never block scene load.
- **Shader reuse:** *(as shipped)* no separate denoise shaders were duplicated or refactored into a library — temporal accumulation is inlined in `lumen_trace.wgsl`.

---

## Critical files

| Purpose | Path |
|---|---|
| Plugin pattern (lib.rs, sync, inspector) | `crates/renzora_rt/src/lib.rs` |
| Multi-pass compute node template | `crates/renzora_rt/src/node.rs` |
| Component + quality + push constants template | `crates/renzora_rt/src/settings.rs` |
| Voxel radiance clipmap (4-cascade cache) | `crates/renzora_lumen/src/voxel_cache.rs` |
| Runtime CPU voxelization of scene geometry | `crates/renzora_lumen/src/geometry_voxelize.rs` |
| Voxel-cone GI tracer (+ inlined temporal) | `crates/renzora_lumen/src/lumen_trace.rs`, `lumen_trace.wgsl` |
| Screen-space reflection pyramid (Phase 10d) | `crates/renzora_lumen/src/screen_reflection{,_blur,_resolve}.rs` |
| EffectRouting definition | `crates/renzora/src/core/mod.rs:276-293` |
| Plugin registration site | `crates/renzora_runtime/src/lib.rs:112` |
| wgpu features (for future HWRT gate) | `crates/renzora_runtime/src/lib.rs:47-65` |
| Workspace members (add crate here) | `Cargo.toml:2-49` |

---

## Verification

Each phase has its own check:

- **Phase 1:** `cargo check -p renzora_lumen` passes; running the editor shows a "Lumen GI" entry; `ScreenSpace` quality = visually identical to old `RtLighting`.
- **Phase 2:** enable `VoxelCache` debug view; rotate camera — voxel colors track direct lighting from sun + point lights.
- **Phase 3:** *(abandoned — no SDF bake)* instead: enable the `VoxelCache` debug view and confirm scene geometry appears voxelized in the clipmap cascades (CPU voxelization in `geometry_voxelize.rs`).
- **Phase 4:** *(abandoned — no global SDF)* instead: move the camera and confirm the voxel clipmap cascades scroll/snap correctly and recycle voxels crossing cascade boundaries.
- **Phase 5:** place a brightly-colored object off-screen above the camera; nearby on-screen surfaces should pick up its color via voxel-cone trace (SSGI fails this test, Lumen passes).
- **Phase 6:** quick camera cut (`reset = true`) — GI should snap clean instead of ghosting; continuous motion should be stable under the inlined temporal accumulation.
- **Phase 7:** *(abandoned — no emissive injection)*.
- **Phase 8:** point an upward-facing surface at open sky with no nearby voxel content — it should pick up ambient sky bounce (sky-cubemap cone-miss fallback). Reflections (Phase 10d) verify separately: wet-floor scene shows the screen-space reflection pyramid.
- **Phase 9:** quality dropdown sweeps end-to-end; debug views all functional; pass timings plausible.

Acceptance for "ship default": Phase 6 complete, running on at least one integrated GPU at 60 FPS 1080p Medium, no crashes / validation errors in 10-minute playthrough of a representative scene.
