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

- **Camera.** `SolariGi` is routed from the World Environment onto each active
  camera via `EffectRouting` (the same path Lumen uses). On a routed camera the
  plugin inserts Bevy's `SolariLighting` — whose `#[require]`s pull in HDR and the
  deferred/depth/motion-vector prepasses — and forces **`Msaa::Off`** (Solari
  mandates no MSAA). Disabling it removes `SolariLighting` and restores the
  default MSAA.
- **Meshes.** While Solari is active, conforming meshes are mirrored into the
  ray-tracing scene with `RaytracingMesh3d` (it coexists with the rasterized
  `Mesh3d`). Solari's BLAS builder requires `TriangleList` topology, 32-bit
  indices, and the `{POSITION, NORMAL, UV_0, TANGENT}` vertex attributes — many
  imported GLBs lack tangents, so **non-conforming meshes are skipped** (marked so
  they aren't re-checked every frame) instead of crashing the builder. Only
  `MeshMaterial3d<StandardMaterial>` entities participate.
- **Idle.** When no camera has Solari enabled, the ray-tracing mirror is dropped
  so BLAS resources are freed.

## Limitations

- RT-capable GPU + Vulkan/DX12/Metal backend only (never GL/web/Android).
- `StandardMaterial` only; custom-WGSL materials are not traced.
- Meshes without tangents/UVs (or with 16-bit indices) are not lit by Solari.
- Solari and Lumen are **mutually exclusive per camera** — don't author both
  `SolariGi` and `LumenLighting` on the same World Environment.

## Plugin ABI note

Enabling the `bevy_solari` Bevy feature recompiles the shared `bevy_dylib`, which
**moves the plugin ABI hash** (see [the plugin ABI section](../extending/plugins.md)
and `CLAUDE.md` §3). Every existing distribution plugin must be rebuilt against
the new dylib, and the pinned ABI hash is re-pinned to the new value after the
build.
